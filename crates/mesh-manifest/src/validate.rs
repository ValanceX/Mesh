//! Checks a parsed manifest document against the version 1 format and
//! the model's own rules, and builds the [`Manifest`].
//!
//! The loader's phases (phase 1, JSON syntax, runs in `json.rs`; phase 6,
//! template selection, is [`Manifest::template`](crate::Manifest::template)):
//! 2. **Version gate.** A root that isn't an object, or a missing or
//!    unsupported `"version"`, is the only error reported.
//! 3. **Structure:** JSON types, required and unknown properties,
//!    repeated keys, kinds, and an `optional` directly wrapping an
//!    `optional`.
//! 4. **Declarations:** name rules, and repeated parameter names.
//! 5. **Named-type graph**, over every declared named type: unknown
//!    references, recursion, and an `optional` wrapping a named type that
//!    expands to `optional`.
//!
//! Phases 3 and 4 run as one traversal of the document; phase 5 runs
//! after it. Every problem they find is reported. A value that is wrong is
//! reported once and nothing inside it is checked, so one mistake doesn't
//! cascade; its siblings are still checked. Diagnostics are sorted by
//! start byte, and ties keep emission order (the traversal before phase
//! 5, document order within each). The [`Manifest`] is built only if
//! nothing was reported.

use crate::json::{Json, Member, Value};
use crate::{
    Command, Component, Constant, DeclarationKey, Effect, Event, Field, Manifest, Parameter, Type,
    Unit, UnitField, MAX_ENUM_VALUES, VERSION,
};
use mesh_syntax::{Diagnostic, DiagnosticCode, Severity, Span};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(crate) fn manifest(document: &Json) -> Result<Manifest, Vec<Diagnostic>> {
    let mut validator = Validator::default();
    let manifest = validator.document(document);
    if validator.diagnostics.is_empty() {
        Ok(manifest.expect("a manifest with no diagnostics was built"))
    } else {
        // Checks run section by section; report in source order.
        validator.diagnostics.sort_by_key(|d| d.span.start_byte);
        Err(validator.diagnostics)
    }
}

/// What a declared name must look like.
#[derive(Clone, Copy)]
enum NameRule {
    /// A component: an MPRX tag name, such as `user-card`.
    Tag,
    /// Anything else: an MPRX identifier, such as `selectUser`.
    Identifier,
}

/// Words MPRX reads as literals, so no identifier can be one.
const RESERVED: [&str; 3] = ["true", "false", "null"];

/// A `named` type reference, checked once every named type is known.
struct NamedRef {
    name: String,
    /// The reference's `"name"` value.
    span: Span,
    /// The named type whose definition contains the reference, if any.
    in_definition: Option<String>,
    /// Whether an `optional` type directly wraps the reference.
    wrapped_in_optional: bool,
}

/// A prop's `default`, checked against the prop's type once every named
/// type is known.
struct PendingDefault {
    prop: String,
    /// The `"default"` value's span.
    span: Span,
    value: Constant,
    ty: Type,
}

#[derive(Default)]
struct Validator {
    defaults: Vec<PendingDefault>,
    /// Whether the next type read may be a `snippet` (a component's prop or scope name; an
    /// `optional` wrapping it passes this on). Taken at once by the type read.
    snippet_allowed: bool,
    diagnostics: Vec<Diagnostic>,
    /// Every named type declared, first occurrence only, in source order,
    /// with its key's span. Declared means a reference to it isn't
    /// unknown, even if its definition has errors.
    declared_types: Vec<(String, Span)>,
    /// The named types whose definitions are valid.
    types: BTreeMap<String, Type>,
    refs: Vec<NamedRef>,
    /// The named type being defined while its definition is checked.
    in_definition: Option<String>,
    /// Where each declaration's key is. Only complete if nothing was
    /// reported, which is the only case a `Manifest` is built in.
    spans: BTreeMap<DeclarationKey, Span>,
    /// Every name the manifest's `context` declares, so that a `uses` of one whose type has
    /// errors isn't also reported as unknown.
    declared_context: BTreeSet<String>,
    /// The context names whose types are valid, with their types.
    context: BTreeMap<String, Type>,
}

/// An object's properties by key, first occurrence only.
struct Properties<'j> {
    members: Vec<&'j Member>,
}

impl<'j> Properties<'j> {
    fn get(&self, key: &str) -> Option<&'j Json> {
        self.members
            .iter()
            .find(|member| member.key == key)
            .map(|member| &member.value)
    }
}

/// The first byte of a value: an object's `{`. A missing property is
/// reported here rather than across the whole object.
fn opening(json: &Json) -> Span {
    Span {
        start_byte: json.span.start_byte,
        end_byte: json.span.start_byte + 1,
    }
}

impl Validator {
    fn error(&mut self, code: DiagnosticCode, span: Span, message: String) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            message,
            span,
            suggestions: Vec::new(),
        });
    }

    fn document(&mut self, json: &Json) -> Option<Manifest> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "a manifest must be a JSON object, found {}",
                    json.value.describe()
                ),
            );
            return None;
        };

        // A manifest of another version has another schema, so nothing
        // else in it can be checked.
        let Some(version) = members.iter().find(|member| member.key == "version") else {
            self.error(
                DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                opening(json),
                format!("the manifest has no \"version\"; this MESH reads version {VERSION}"),
            );
            return None;
        };
        match &version.value.value {
            Value::Number(number) if number.as_u64() == Some(VERSION) => {}
            Value::Number(number) => {
                self.error(
                    DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                    version.value.span,
                    format!(
                        "unsupported manifest version {number}; this MESH reads version {VERSION}"
                    ),
                );
                return None;
            }
            other => {
                self.error(
                    DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                    version.value.span,
                    format!(
                        "the manifest version must be the number {VERSION}, found {}",
                        other.describe()
                    ),
                );
                return None;
            }
        }

        let properties = self.object(
            json,
            "the manifest",
            &["version", "types", "components"],
            &["$schema", "context"],
        )?;
        if let Some(schema) = properties.get("$schema") {
            self.string(schema, "\"$schema\"");
        }

        if let Some(types) = properties.get("types") {
            for member in self.names(
                types,
                "types",
                "type",
                "the manifest's \"types\"",
                NameRule::Identifier,
            ) {
                self.declared_types
                    .push((member.key.clone(), member.key_span));
                self.declare(
                    DeclarationKey::NamedType(member.key.clone()),
                    member.key_span,
                );
                self.declare_fields(&member.key, &member.value);
                self.in_definition = Some(member.key.clone());
                if let Some(ty) = self.ty(&member.value) {
                    self.types.insert(member.key.clone(), ty);
                }
                self.in_definition = None;
            }
        }

        // The context channel: names a host supplies beside the snapshot, each with its type. Read
        // before the components, whose `uses` name them.
        if let Some(json) = properties.get("context") {
            for member in self.names(
                json,
                "context",
                "context name",
                "the manifest's \"context\"",
                NameRule::Identifier,
            ) {
                self.declared_context.insert(member.key.clone());
                self.declare(DeclarationKey::Context(member.key.clone()), member.key_span);
                self.snippet_allowed = false;
                if let Some(ty) = self.ty(&member.value) {
                    self.context.insert(member.key.clone(), ty);
                }
            }
        }

        let mut components = Some(BTreeMap::new());
        if let Some(json) = properties.get("components") {
            for member in self.names(
                json,
                "components",
                "component",
                "\"components\"",
                NameRule::Tag,
            ) {
                self.declare(
                    DeclarationKey::Component(member.key.clone()),
                    member.key_span,
                );
                let component = self.component(&member.key, &member.value);
                insert(&mut components, &member.key, component);
            }
        }

        self.check_named_types();
        self.check_defaults();

        let components_span = members
            .iter()
            .find(|member| member.key == "components")
            .map(|member| member.key_span)?;
        Some(Manifest {
            types: std::mem::take(&mut self.types),
            context: std::mem::take(&mut self.context),
            components: components?,
            components_span,
            spans: std::mem::take(&mut self.spans),
        })
    }

    fn component(&mut self, name: &str, json: &Json) -> Option<Component> {
        let what = format!("component {name:?}");
        let properties = self.object(
            json,
            &what,
            &["props", "events", "commands", "scope"],
            &["unit", "uses"],
        )?;
        let owner = format!("component {name:?}");

        let mut props = Some(BTreeMap::new());
        if let Some(json) = properties.get("props") {
            for member in self.names(json, "props", "prop", &owner, NameRule::Identifier) {
                let key = DeclarationKey::Prop(name.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                let field = self.field(&member.value, &format!("prop {:?}", member.key), true);
                // `children` is the content a caller writes inside the component: a snippet with no
                // parameters.
                if member.key == "children" {
                    if let Some(field) = &field {
                        let inner = match &field.ty {
                            Type::Optional(inner) => inner.as_ref(),
                            other => other,
                        };
                        if !matches!(inner, Type::Snippet(params) if params.is_empty()) {
                            self.error(
                                DiagnosticCode::MANIFEST_CHILDREN_NOT_SNIPPET,
                                member.key_span,
                                "the prop `children` is the content written inside the component, so its type must be a snippet with no parameters: { \"kind\": \"snippet\" }".to_string(),
                            );
                        }
                    }
                }
                insert(&mut props, &member.key, field);
            }
        }

        let mut events = Some(BTreeMap::new());
        if let Some(json) = properties.get("events") {
            for member in self.names(json, "events", "event", &owner, NameRule::Identifier) {
                let key = DeclarationKey::Event(name.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                let event = self.event(&member.value, &member.key);
                insert(&mut events, &member.key, event);
            }
        }

        let mut commands = Some(BTreeMap::new());
        if let Some(json) = properties.get("commands") {
            for member in self.names(json, "commands", "command", &owner, NameRule::Identifier) {
                let key = DeclarationKey::Command(name.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                let command = self.command(&member.value, name, &member.key);
                insert(&mut commands, &member.key, command);
            }
        }

        let mut scope = Some(BTreeMap::new());
        if let Some(json) = properties.get("scope") {
            for member in self.names(json, "scope", "scope name", &owner, NameRule::Identifier) {
                let key = DeclarationKey::Scope(name.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                self.snippet_allowed = true;
                let ty = self.ty(&member.value);
                insert(&mut scope, &member.key, ty);
            }
        }

        let mut props = props?;
        let mut commands = commands?;
        let mut scope = scope?;
        let unit = match properties.get("unit") {
            Some(json) => self.unit(json, name, &props, &mut commands, &mut scope),
            None => None,
        };
        let uses = match properties.get("uses") {
            Some(json) => self.uses(json, name, &props, &mut scope),
            None => BTreeSet::new(),
        };

        Some(Component {
            props: std::mem::take(&mut props),
            events: events?,
            commands,
            scope,
            unit,
            uses,
        })
    }

    /// A component's `uses`: an array of names the manifest's `context` declares. Each joins
    /// `scope` with the context's type, read-only (MPRX has no assignment), so a template reads
    /// it like any scope name and is type-checked the same way before it runs.
    fn uses(
        &mut self,
        json: &Json,
        component: &str,
        props: &BTreeMap<String, Field>,
        scope: &mut BTreeMap<String, Type>,
    ) -> BTreeSet<String> {
        let mut uses = BTreeSet::new();
        let Value::Array(items) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "\"uses\" must be an array of context names, found {}",
                    json.value.describe()
                ),
            );
            return uses;
        };
        for item in items {
            let Some(name) = self.string(item, "a name in \"uses\"") else {
                continue;
            };
            if !self.declared_context.contains(&name) {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_CONTEXT,
                    item.span,
                    format!(
                        "component {component:?} uses `{name}`, which the manifest's \"context\" doesn't declare"
                    ),
                );
                continue;
            }
            let clash = if uses.contains(&name) {
                Some("already in its `uses`")
            } else if scope.contains_key(&name) {
                Some("a scope name of the component")
            } else if props.contains_key(&name) {
                Some("a prop of the component")
            } else {
                None
            };
            if let Some(clash) = clash {
                self.error(
                    DiagnosticCode::MANIFEST_CONTEXT_CLASH,
                    item.span,
                    format!("the context name `{name}` is {clash}"),
                );
                continue;
            }
            // A context name whose type has errors was reported where it is declared.
            let Some(ty) = self.context.get(&name).cloned() else {
                continue;
            };
            self.declare(
                DeclarationKey::Scope(component.to_string(), name.clone()),
                item.span,
            );
            scope.insert(name.clone(), ty);
            uses.insert(name);
        }
        uses
    }

    /// A component's `unit`: `{ "state": { name: { "type", "default", "controllable"? } }, "commands": { name:
    /// { "parameters": [...] } } }`. The state names join `scope`, and the commands join `commands`.
    fn unit(
        &mut self,
        json: &Json,
        component: &str,
        props: &BTreeMap<String, Field>,
        commands: &mut BTreeMap<String, Command>,
        scope: &mut BTreeMap<String, Type>,
    ) -> Option<Unit> {
        let what = format!("the unit of component {component:?}");
        let properties = self.object(json, &what, &[], &["state", "commands"])?;
        let owner = what.clone();
        let mut state = BTreeMap::new();
        let mut names = BTreeSet::new();

        if let Some(json) = properties.get("state") {
            for member in self.names(json, "state", "state field", &owner, NameRule::Identifier) {
                let label = format!("state field {:?}", member.key);
                let Some(fields) = self.object(
                    &member.value,
                    &label,
                    &["type", "default"],
                    &["controllable"],
                ) else {
                    continue;
                };
                self.snippet_allowed = false;
                let Some(ty) = fields.get("type").and_then(|json| self.ty(json)) else {
                    continue;
                };
                let default = fields
                    .get("default")
                    .map(|json| (json.span, constant(json)));
                let Some((span, default)) = default else {
                    continue;
                };
                self.defaults.push(PendingDefault {
                    prop: label.clone(),
                    span,
                    value: default.clone(),
                    ty: ty.clone(),
                });
                let controllable = match fields.get("controllable") {
                    None => false,
                    Some(json) => match json.value {
                        Value::Bool(value) => value,
                        ref other => {
                            self.error(
                                DiagnosticCode::MANIFEST_INVALID_VALUE,
                                json.span,
                                format!(
                                    "\"controllable\" must be true or false, found {}",
                                    other.describe()
                                ),
                            );
                            continue;
                        }
                    },
                };
                let clash = if scope.contains_key(&member.key) {
                    Some("a scope name of the component".to_string())
                } else if let Some(prop) = props.get(&member.key) {
                    let same = prop.ty == ty || prop.ty == Type::Optional(Box::new(ty.clone()));
                    if controllable && !prop.required && same {
                        None
                    } else if controllable {
                        Some(
                            "a prop of the component that is required or of another type"
                                .to_string(),
                        )
                    } else {
                        Some("a prop of the component (only a `controllable` field may share a prop's name)".to_string())
                    }
                } else {
                    None
                };
                if let Some(clash) = clash {
                    self.error(
                        DiagnosticCode::MANIFEST_UNIT_CLASH,
                        member.key_span,
                        format!("the unit's state field `{}` is also {clash}", member.key),
                    );
                    continue;
                }
                let key = DeclarationKey::Scope(component.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                scope.insert(member.key.clone(), ty.clone());
                state.insert(
                    member.key.clone(),
                    UnitField {
                        ty,
                        default,
                        controllable,
                    },
                );
            }
        }

        if let Some(json) = properties.get("commands") {
            for member in self.names(json, "commands", "command", &owner, NameRule::Identifier) {
                if commands.contains_key(&member.key) || scope.contains_key(&member.key) {
                    self.error(
                        DiagnosticCode::MANIFEST_UNIT_CLASH,
                        member.key_span,
                        format!(
                            "the unit's command `{}` is also a command or a scope name of the component",
                            member.key
                        ),
                    );
                    continue;
                }
                let key = DeclarationKey::Command(component.to_string(), member.key.clone());
                self.declare(key, member.key_span);
                if let Some(command) = self.command(&member.value, component, &member.key) {
                    commands.insert(member.key.clone(), command);
                    names.insert(member.key.clone());
                }
            }
        }

        Some(Unit {
            state,
            commands: names,
        })
    }

    fn event(&mut self, json: &Json, name: &str) -> Option<Event> {
        let properties = self.object(json, &format!("event {name:?}"), &[], &["payload"])?;
        let payload = match properties.get("payload") {
            Some(json) => Some(self.ty(json)?),
            None => None,
        };
        Some(Event { payload })
    }

    fn command(&mut self, json: &Json, component: &str, name: &str) -> Option<Command> {
        let properties = self.object(
            json,
            &format!("command {name:?}"),
            &["parameters"],
            &["effect", "title"],
        )?;
        let effect = properties.get("effect").and_then(|json| {
            let text = self.string(json, "a command's \"effect\"")?;
            match text.as_str() {
                "read" => Some(Effect::Read),
                "write" => Some(Effect::Write),
                "destructive" => Some(Effect::Destructive),
                other => {
                    self.error(
                        DiagnosticCode::MANIFEST_INVALID_VALUE,
                        json.span,
                        format!("a command's \"effect\" is \"read\", \"write\" or \"destructive\", found {other:?}"),
                    );
                    None
                }
            }
        });
        let title = properties
            .get("title")
            .and_then(|json| self.string(json, "a command's \"title\""));
        let json = properties.get("parameters")?;
        let Value::Array(items) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "\"parameters\" must be an array, found {}",
                    json.value.describe()
                ),
            );
            return None;
        };

        let mut parameters = Some(Vec::new());
        let mut seen = HashSet::new();
        for (index, item) in items.iter().enumerate() {
            if let Some(name_json) = item_name(item) {
                let key = DeclarationKey::Parameter(component.to_string(), name.to_string(), index);
                self.declare(key, name_json.span);
            }
            let parameter = self.parameter(item, name, &mut seen);
            match (&mut parameters, parameter) {
                (Some(parameters), Some(parameter)) => parameters.push(parameter),
                _ => parameters = None,
            }
        }
        Some(Command {
            parameters: parameters?,
            effect,
            title,
        })
    }

    fn parameter(
        &mut self,
        json: &Json,
        command: &str,
        seen: &mut HashSet<String>,
    ) -> Option<Parameter> {
        let what = format!("a parameter of command {command:?}");
        let properties = self.object(json, &what, &["name", "type"], &[])?;
        let name_json = properties.get("name");
        let name = name_json.and_then(|json| self.string(json, "a parameter's \"name\""));
        if let (Some(name), Some(json)) = (&name, name_json) {
            if self.valid_name(name, json.span, "parameter", NameRule::Identifier)
                && !seen.insert(name.clone())
            {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_PARAMETER,
                    json.span,
                    format!("command {command:?} has two parameters named {name:?}"),
                );
            }
        }
        let ty = properties.get("type").and_then(|json| self.ty(json));
        Some(Parameter {
            name: name?,
            ty: ty?,
        })
    }

    /// A prop or record field declaration: `{ "type": ..., "required": ... }`,
    /// and, for a prop (`is_prop`), an optional `"default"`.
    fn field(&mut self, json: &Json, what: &str, is_prop: bool) -> Option<Field> {
        let optional: &[&str] = if is_prop { &["default"] } else { &[] };
        let properties = self.object(json, what, &["type", "required"], optional)?;
        let ty = properties.get("type").and_then(|json| {
            // A prop may be a snippet; a record's field may not.
            self.snippet_allowed = is_prop;
            self.ty(json)
        });
        let required = properties
            .get("required")
            .and_then(|json| match json.value {
                Value::Bool(required) => Some(required),
                ref other => {
                    self.error(
                        DiagnosticCode::MANIFEST_INVALID_VALUE,
                        json.span,
                        format!(
                            "\"required\" must be true or false, found {}",
                            other.describe()
                        ),
                    );
                    None
                }
            });
        let default = properties
            .get("default")
            .map(|json| (json.span, constant(json)));
        let ty = ty?;
        let default = match default {
            Some((span, value)) => {
                self.defaults.push(PendingDefault {
                    prop: what.to_string(),
                    span,
                    value: value.clone(),
                    ty: ty.clone(),
                });
                Some(value)
            }
            None => None,
        };
        Some(Field {
            ty,
            required: required?,
            default,
        })
    }

    /// Records where a declaration is, keeping the first occurrence. A
    /// repeated key is an error anyway, so no manifest is built from it.
    fn declare(&mut self, key: DeclarationKey, span: Span) {
        self.spans.entry(key).or_insert(span);
    }

    /// Records the fields of named type `name`, if its definition `json`
    /// is a record. Only the definition's own fields are addressable: a
    /// record nested deeper has no name to find it by.
    fn declare_fields(&mut self, name: &str, json: &Json) {
        let Value::Object(members) = &json.value else {
            return;
        };
        let is_record = members.iter().any(|member| {
            member.key == "kind"
                && matches!(&member.value.value, Value::String(kind) if kind == "record")
        });
        let fields = members.iter().find(|member| member.key == "fields");
        if let (true, Some(fields)) = (is_record, fields) {
            if let Value::Object(fields) = &fields.value.value {
                for field in fields {
                    let key = DeclarationKey::Field(name.to_string(), field.key.clone());
                    self.declare(key, field.key_span);
                }
            }
        }
    }

    fn ty(&mut self, json: &Json) -> Option<Type> {
        self.ty_in(json, false)
    }

    /// A type. `in_optional` is whether an `optional` type directly wraps
    /// this one.
    fn ty_in(&mut self, json: &Json, in_optional: bool) -> Option<Type> {
        let snippet_allowed = std::mem::take(&mut self.snippet_allowed);
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "a type must be an object with a \"kind\", found {}",
                    json.value.describe()
                ),
            );
            return None;
        };
        let Some(kind) = members.iter().find(|member| member.key == "kind") else {
            self.error(
                DiagnosticCode::MANIFEST_MISSING_PROPERTY,
                opening(json),
                "a type must have a \"kind\"".to_string(),
            );
            return None;
        };
        let Value::String(kind_name) = &kind.value.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                kind.value.span,
                format!(
                    "\"kind\" must be a string, found {}",
                    kind.value.value.describe()
                ),
            );
            self.repeated_kinds(members);
            return None;
        };

        let primitive = match kind_name.as_str() {
            "string" => Some(Type::String),
            "number" => Some(Type::Number),
            "boolean" => Some(Type::Boolean),
            "null" => Some(Type::Null),
            "any" => Some(Type::Any),
            _ => None,
        };
        if let Some(primitive) = primitive {
            self.object(json, &format!("a {kind_name} type"), &["kind"], &[])?;
            return Some(primitive);
        }

        match kind_name.as_str() {
            "list" => {
                let properties = self.object(json, "a list type", &["kind", "element"], &[])?;
                let element = self.ty(properties.get("element")?)?;
                Some(Type::List(Box::new(element)))
            }
            "record" => {
                let properties = self.object(json, "a record type", &["kind", "fields"], &[])?;
                let fields_json = properties.get("fields")?;
                let mut fields = Some(BTreeMap::new());
                for member in self.names(
                    fields_json,
                    "fields",
                    "field",
                    "a record type",
                    NameRule::Identifier,
                ) {
                    let field =
                        self.field(&member.value, &format!("field {:?}", member.key), false);
                    insert(&mut fields, &member.key, field);
                }
                Some(Type::Record(fields?))
            }
            "named" => {
                let properties = self.object(json, "a named type", &["kind", "name"], &[])?;
                let name_json = properties.get("name")?;
                let name = self.string(name_json, "a named type's \"name\"")?;
                self.refs.push(NamedRef {
                    name: name.clone(),
                    span: name_json.span,
                    in_definition: self.in_definition.clone(),
                    wrapped_in_optional: in_optional,
                });
                Some(Type::Named(name))
            }
            "enum" => {
                let properties = self.object(json, "an enum type", &["kind", "values"], &[])?;
                let values_json = properties.get("values")?;
                let Value::Array(items) = &values_json.value else {
                    self.error(
                        DiagnosticCode::MANIFEST_INVALID_VALUE,
                        values_json.span,
                        format!(
                            "\"values\" must be an array of strings, found {}",
                            values_json.value.describe()
                        ),
                    );
                    return None;
                };
                if items.is_empty() {
                    self.error(
                        DiagnosticCode::MANIFEST_INVALID_VALUE,
                        values_json.span,
                        "an enum type needs at least one value".to_string(),
                    );
                    return None;
                }
                let mut values = Some(Vec::new());
                let mut seen = HashSet::new();
                for item in items {
                    let Value::String(text) = &item.value else {
                        self.error(
                            DiagnosticCode::MANIFEST_INVALID_VALUE,
                            item.span,
                            format!(
                                "an enum value must be a string, found {}",
                                item.value.describe()
                            ),
                        );
                        values = None;
                        continue;
                    };
                    if !seen.insert(text.as_str()) {
                        self.error(
                            DiagnosticCode::MANIFEST_DUPLICATE_VARIANT,
                            item.span,
                            format!("the enum lists the value {text:?} twice"),
                        );
                        values = None;
                    } else if let Some(values) = &mut values {
                        values.push(text.clone());
                    }
                }
                if items.len() > MAX_ENUM_VALUES {
                    self.error(
                        DiagnosticCode::MANIFEST_ENUM_TOO_LARGE,
                        values_json.span,
                        format!(
                            "an enum has {} values, more than the {MAX_ENUM_VALUES} MESH supports; keep a larger set as `string` and validate it in the application",
                            items.len()
                        ),
                    );
                    return None;
                }
                Some(Type::Enum(values?))
            }
            "snippet" => {
                let properties = self.object(json, "a snippet type", &["kind"], &["params"])?;
                if !snippet_allowed {
                    self.error(
                        DiagnosticCode::MANIFEST_SNIPPET_MISPLACED,
                        kind.value.span,
                        "a snippet type can only be the type of a component's prop or scope name: it is never stored, returned or sent, so it is not a record field, a list element, a named type, an event payload or a command parameter, and not a snippet's own parameter".to_string(),
                    );
                    return None;
                }
                let mut params = Some(Vec::new());
                if let Some(list) = properties.get("params") {
                    let Value::Array(items) = &list.value else {
                        self.error(
                            DiagnosticCode::MANIFEST_INVALID_VALUE,
                            list.span,
                            format!(
                                "\"params\" must be an array of types, found {}",
                                list.value.describe()
                            ),
                        );
                        return None;
                    };
                    for item in items {
                        match (self.ty(item), &mut params) {
                            (Some(ty), Some(params)) => params.push(ty),
                            _ => params = None,
                        }
                    }
                }
                Some(Type::Snippet(params?))
            }
            "optional" => {
                let properties = self.object(json, "an optional type", &["kind", "type"], &[])?;
                let inner_json = properties.get("type")?;
                self.snippet_allowed = snippet_allowed;
                let inner = self.ty_in(inner_json, true)?;
                if let Type::Optional(_) = inner {
                    self.error(
                        DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
                        inner_json.span,
                        "an optional type can't wrap another optional type".to_string(),
                    );
                    return None;
                }
                Some(Type::Optional(Box::new(inner)))
            }
            "void" | "nothing" => {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_KIND,
                    kind.value.span,
                    format!("`{kind_name}` is internal to MESH; a manifest can't write it"),
                );
                self.repeated_kinds(members);
                None
            }
            _ => {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_KIND,
                    kind.value.span,
                    format!(
                        "unknown type kind {kind_name:?}; expected string, number, boolean, \
                         null, any, list, record, named, optional, enum or snippet"
                    ),
                );
                self.repeated_kinds(members);
                None
            }
        }
    }

    /// Reports every `"kind"` after the first in a type object whose
    /// first `"kind"` is invalid. Such an object's other properties aren't
    /// checked, but a repeated `"kind"` is still reported, so it can't
    /// look as though the later one was used. (With a valid first
    /// `"kind"`, [`Validator::object`] reports repeats.)
    fn repeated_kinds(&mut self, members: &[Member]) {
        for member in members.iter().filter(|member| member.key == "kind").skip(1) {
            self.error(
                DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                member.key_span,
                "a type repeats the property \"kind\"".to_string(),
            );
        }
    }

    /// Checks that `json` is an object with every `required` property,
    /// no property outside `required` and `optional`, and no repeated key.
    /// Returns `None` only if `json` isn't an object at all.
    fn object<'j>(
        &mut self,
        json: &'j Json,
        what: &str,
        required: &[&str],
        optional: &[&str],
    ) -> Option<Properties<'j>> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!("{what} must be an object, found {}", json.value.describe()),
            );
            return None;
        };

        let mut first = Vec::new();
        let mut seen = HashSet::new();
        for member in members {
            if !seen.insert(member.key.as_str()) {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                    member.key_span,
                    format!("{what} repeats the property {:?}", member.key),
                );
            } else if required.contains(&member.key.as_str())
                || optional.contains(&member.key.as_str())
            {
                first.push(member);
            } else {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY,
                    member.key_span,
                    format!("{what} can't have the property {:?}", member.key),
                );
            }
        }
        for property in required {
            if !seen.contains(property) {
                self.error(
                    DiagnosticCode::MANIFEST_MISSING_PROPERTY,
                    opening(json),
                    format!("{what} is missing the property {property:?}"),
                );
            }
        }
        Some(Properties { members: first })
    }

    /// Checks that `json`, the value of `property`, is an object mapping
    /// declared names to declarations: each name valid under `rule`, and
    /// none repeated.
    /// Returns the members to check further: the first occurrence of each
    /// name.
    fn names<'j>(
        &mut self,
        json: &'j Json,
        property: &str,
        noun: &str,
        owner: &str,
        rule: NameRule,
    ) -> Vec<&'j Member> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "{property:?} must be an object, found {}",
                    json.value.describe()
                ),
            );
            return Vec::new();
        };

        let mut first = Vec::new();
        let mut seen = HashSet::new();
        for member in members {
            if !seen.insert(member.key.as_str()) {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                    member.key_span,
                    format!("{owner} declares the {noun} {:?} twice", member.key),
                );
                continue;
            }
            self.valid_name(&member.key, member.key_span, noun, rule);
            first.push(member);
        }
        first
    }

    /// Reports `name` if MPRX can't write it where a `noun` goes.
    fn valid_name(&mut self, name: &str, span: Span, noun: &str, rule: NameRule) -> bool {
        let mut chars = name.chars();
        let starts_well = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        let rest_ok = chars.all(|c| match rule {
            NameRule::Tag => c.is_ascii_alphanumeric() || c == '_' || c == '-',
            NameRule::Identifier => c.is_ascii_alphanumeric() || c == '_',
        });
        let message = if !(starts_well && rest_ok) {
            match rule {
                NameRule::Tag => format!(
                    "{noun} name {name:?} isn't a valid MPRX tag name: start with a letter \
                     or `_`, then use letters, digits, `_` or `-`"
                ),
                NameRule::Identifier => format!(
                    "{noun} name {name:?} isn't a valid MPRX identifier: start with a letter \
                     or `_`, then use letters, digits or `_`"
                ),
            }
        } else if matches!(rule, NameRule::Identifier) && RESERVED.contains(&name) {
            format!("{noun} name {name:?} is reserved: MPRX reads it as a literal")
        } else {
            return true;
        };
        self.error(DiagnosticCode::MANIFEST_INVALID_NAME, span, message);
        false
    }

    fn string(&mut self, json: &Json, what: &str) -> Option<String> {
        match &json.value {
            Value::String(value) => Some(value.clone()),
            other => {
                self.error(
                    DiagnosticCode::MANIFEST_INVALID_VALUE,
                    json.span,
                    format!("{what} must be a string, found {}", other.describe()),
                );
                None
            }
        }
    }

    /// The checks that need every named type: references resolve, no
    /// optional wraps a named type that is already optional, and no named
    /// type refers to itself.
    fn check_named_types(&mut self) {
        let declared: BTreeSet<String> = self
            .declared_types
            .iter()
            .map(|(name, _)| name.clone())
            .collect();

        let refs = std::mem::take(&mut self.refs);

        // Each cycle is reported once, at the first of its types in the
        // source.
        let mut recursive = HashSet::new();
        for (name, key_span) in self.declared_types.clone() {
            if recursive.contains(&name) {
                continue;
            }
            if let Some(cycle) = find_cycle(&name, &refs) {
                self.error(
                    DiagnosticCode::MANIFEST_RECURSIVE_TYPE,
                    key_span,
                    format!("type {name:?} refers to itself: {}", cycle.join(" -> ")),
                );
                recursive.extend(cycle);
            }
        }

        for reference in &refs {
            if !declared.contains(&reference.name) {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_TYPE,
                    reference.span,
                    format!(
                        "unknown type {:?}: the manifest's \"types\" doesn't declare it",
                        reference.name
                    ),
                );
            } else if reference.wrapped_in_optional
                // A recursive type has been reported already.
                && !recursive.contains(&reference.name)
                && self.is_optional(&reference.name)
            {
                self.error(
                    DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
                    reference.span,
                    format!(
                        "an optional type can't wrap {:?}, which is already optional",
                        reference.name
                    ),
                );
            }
        }
    }

    /// Every prop's `default` must be a value of the prop's type (an
    /// optional prop's default is a value of the type inside the `optional`).
    /// A type that is unknown or invalid has been reported already.
    fn check_defaults(&mut self) {
        for pending in std::mem::take(&mut self.defaults) {
            let ty = match self.expand(&pending.ty) {
                Some(Type::Optional(inner)) => Some(inner.as_ref().clone()),
                other => other.cloned(),
            };
            let Some(ty) = ty else { continue };
            if let Err(why) = self.fits(&pending.value, &ty) {
                self.error(
                    DiagnosticCode::MANIFEST_INVALID_VALUE,
                    pending.span,
                    format!(
                        "the default of {} doesn't fit its type: {why}",
                        pending.prop
                    ),
                );
            }
        }
    }

    /// `ty` with named references at the top followed; `None` if one is
    /// unknown or recursive.
    fn expand<'a>(&'a self, ty: &'a Type) -> Option<&'a Type> {
        let mut ty = ty;
        for _ in 0..=self.types.len() {
            match ty {
                Type::Named(name) => ty = self.types.get(name)?,
                other => return Some(other),
            }
        }
        None
    }

    /// Whether `value` is a value of `ty`. An unresolvable type accepts
    /// anything: it has been reported already.
    fn fits(&self, value: &Constant, ty: &Type) -> Result<(), String> {
        let Some(ty) = self.expand(ty) else {
            return Ok(());
        };
        let found = |value: &Constant| match value {
            Constant::Null => "null",
            Constant::Boolean(_) => "a boolean",
            Constant::Number(_) => "a number",
            Constant::String(_) => "a string",
            Constant::List(_) => "a list",
            Constant::Record(_) => "a record",
        };
        match (ty, value) {
            (Type::Any, _)
            | (Type::String, Constant::String(_))
            | (Type::Number, Constant::Number(_))
            | (Type::Boolean, Constant::Boolean(_))
            | (Type::Null, Constant::Null) => Ok(()),
            (Type::Optional(inner), value) => self.fits(value, inner),
            (Type::Enum(values), Constant::String(text)) if values.contains(text) => Ok(()),
            (Type::Enum(values), Constant::String(text)) => Err(format!(
                "{text:?} is not one of {}",
                values
                    .iter()
                    .map(|value| format!("{value:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            (Type::List(element), Constant::List(items)) => {
                items.iter().try_for_each(|item| self.fits(item, element))
            }
            (Type::Record(fields), Constant::Record(record)) => {
                if let Some(name) = record.keys().find(|name| !fields.contains_key(*name)) {
                    return Err(format!("{name:?} is not a field of the record"));
                }
                for (name, field) in fields {
                    match record.get(name) {
                        Some(value) => self.fits(value, &field.ty)?,
                        None if field.required
                            && !matches!(self.expand(&field.ty), Some(Type::Optional(_))) =>
                        {
                            return Err(format!("the field {name:?} is missing"));
                        }
                        None => {}
                    }
                }
                Ok(())
            }
            (ty, value) => Err(format!("{} where {} is needed", found(value), describe(ty))),
        }
    }

    /// Whether named type `name` is, after following named references,
    /// an optional type. An unknown or invalid definition counts as not
    /// optional: it has been reported already.
    fn is_optional(&self, name: &str) -> bool {
        let mut seen = HashSet::new();
        let mut ty = self.types.get(name);
        while let Some(current) = ty {
            match current {
                Type::Optional(_) => return true,
                Type::Named(next) if seen.insert(next.as_str()) => ty = self.types.get(next),
                _ => return false,
            }
        }
        false
    }
}

/// A path of named-type references from `start` back to `start`, such as
/// `["Node", "Children", "Node"]`, if there is one.
fn find_cycle(start: &str, refs: &[NamedRef]) -> Option<Vec<String>> {
    fn visit(
        current: &str,
        start: &str,
        refs: &[NamedRef],
        path: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) -> bool {
        for reference in refs {
            if reference.in_definition.as_deref() != Some(current) {
                continue;
            }
            if reference.name == start {
                path.push(start.to_string());
                return true;
            }
            if visited.insert(reference.name.clone()) {
                path.push(reference.name.clone());
                if visit(&reference.name, start, refs, path, visited) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    let mut path = vec![start.to_string()];
    let mut visited = HashSet::new();
    visit(start, start, refs, &mut path, &mut visited).then_some(path)
}

/// Adds a checked declaration to a map being built. A declaration with
/// errors (`None`) poisons the whole map, since the manifest won't be
/// built anyway.
/// A parameter's `"name"` value, if the parameter is an object that has
/// one, whatever its type: its span is where the parameter is declared.
fn item_name(item: &Json) -> Option<&Json> {
    let Value::Object(members) = &item.value else {
        return None;
    };
    members
        .iter()
        .find(|member| member.key == "name")
        .map(|member| &member.value)
}

fn insert<T>(map: &mut Option<BTreeMap<String, T>>, name: &str, value: Option<T>) {
    match (map.as_mut(), value) {
        (Some(map), Some(value)) => {
            map.insert(name.to_string(), value);
        }
        _ => *map = None,
    }
}

/// A JSON value as a [`Constant`].
fn constant(json: &Json) -> Constant {
    match &json.value {
        Value::Null => Constant::Null,
        Value::Bool(value) => Constant::Boolean(*value),
        Value::Number(number) => Constant::Number(number.as_f64().unwrap_or(f64::MAX)),
        Value::String(text) => Constant::String(text.clone()),
        Value::Array(items) => Constant::List(items.iter().map(constant).collect()),
        Value::Object(members) => Constant::Record(
            members
                .iter()
                .map(|member| (member.key.clone(), constant(&member.value)))
                .collect(),
        ),
    }
}

/// What a type needs, for a message.
fn describe(ty: &Type) -> &'static str {
    match ty {
        Type::String => "a string",
        Type::Number => "a number",
        Type::Boolean => "a boolean",
        Type::Null => "null",
        Type::Any | Type::Named(_) | Type::Optional(_) => "a value",
        Type::List(_) => "a list",
        Type::Record(_) => "a record",
        Type::Enum(_) => "one of its strings",
        Type::Snippet(_) => "a snippet, which has no constant",
    }
}
