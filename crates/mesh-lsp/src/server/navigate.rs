//! Hover and go-to-definition (outline, layer 3: presentation).
//!
//! Everything here is read from facts the compiler already produced: the
//! resolutions and types of a snapshot's analysis, and the manifest's
//! declarations and their spans (D5). Types are printed only through
//! `Ty`'s `Display`, the notation diagnostics use. Nothing here decides
//! what a name means (invariant I1).

use crate::convert::Text;
use lsp_types::{Hover, HoverContents, Location, MarkupContent, MarkupKind, Uri};
use mesh_analysis::{Analysis, Resolution, Target, Ty, Typed};
use mesh_compiler::editor::Recovery;
use mesh_manifest::{Declaration, Manifest};
use mesh_syntax::Span;
use std::str::FromStr;

/// Where a snapshot's answers come from.
#[derive(Clone, Copy)]
pub(super) enum Facts<'a> {
    /// The canonical compile's analysis.
    Canonical(&'a Analysis),
    /// A broken snapshot's editor recovery (outline D3).
    Recovered(&'a Recovery),
}

impl<'a> Facts<'a> {
    fn resolution_at(self, offset: usize) -> Option<&'a Resolution> {
        match self {
            Facts::Canonical(analysis) => analysis.resolution_at(offset),
            Facts::Recovered(recovery) => recovery.resolution_at(offset),
        }
    }

    fn typed_at(self, offset: usize) -> Option<&'a Typed> {
        match self {
            Facts::Canonical(analysis) => analysis.typed_at(offset),
            Facts::Recovered(recovery) => recovery.typed_at(offset),
        }
    }
}

/// The template a snapshot was checked as: its component's name and the
/// manifest that declares it.
#[derive(Clone, Copy)]
pub(super) struct Model<'a> {
    pub(super) component: &'a str,
    pub(super) manifest: &'a Manifest,
}

/// What the cursor at `offset` is on, by the outline's Pass 2 Decision 7:
/// the resolved name if there is one no longer than the typed expression
/// there, otherwise that expression.
enum Hovered<'a> {
    Declaration(&'a Resolution),
    Type(&'a Typed),
}

fn hovered(facts: Facts<'_>, offset: usize) -> Option<Hovered<'_>> {
    let resolution = facts.resolution_at(offset);
    let typed = facts.typed_at(offset);
    match (resolution, typed) {
        (Some(resolution), Some(typed)) if length(resolution.span) <= length(typed.span) => {
            Some(Hovered::Declaration(resolution))
        }
        (Some(resolution), None) => Some(Hovered::Declaration(resolution)),
        (_, Some(typed)) => Some(Hovered::Type(typed)),
        (None, None) => None,
    }
}

fn length(span: Span) -> usize {
    span.end_byte - span.start_byte
}

/// The hover at `offset`, or `None` if nothing there has a type or
/// resolves.
pub(super) fn hover(
    facts: Facts<'_>,
    model: Model<'_>,
    text: Text<'_>,
    offset: usize,
    markdown: bool,
) -> Option<Hover> {
    let (span, block, sentence) = match hovered(facts, offset)? {
        Hovered::Declaration(resolution) => {
            let (block, sentence) = declaration(&resolution.target, model)?;
            (resolution.span, block, sentence)
        }
        Hovered::Type(typed) => (typed.span, typed_block(&typed.ty, model.manifest), None),
    };
    let value = match (markdown, sentence) {
        (true, Some(sentence)) => format!("```\n{block}\n```\n\n{sentence}"),
        (true, None) => format!("```\n{block}\n```"),
        (false, Some(sentence)) => format!("{block}\n\n{sentence}"),
        (false, None) => block,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: if markdown {
                MarkupKind::Markdown
            } else {
                MarkupKind::PlainText
            },
            value,
        }),
        range: Some(text.range(span)),
    })
}

/// The block and sentence describing what `target` names.
fn declaration(target: &Target, model: Model<'_>) -> Option<(String, Option<String>)> {
    let manifest = model.manifest;
    let components = manifest.components();
    Some(match target {
        Target::Component(name) => {
            let component = components.get(name)?;
            let mut lines = Vec::new();
            for (prop, field) in &component.props {
                let mark = if field.required { "" } else { "?" };
                lines.push(format!("  {prop}{mark}: {}", Ty::from(&field.ty)));
            }
            for (event, declared) in &component.events {
                lines.push(format!(
                    "  {}",
                    event_line(event, declared.payload.as_ref())
                ));
            }
            let block = if lines.is_empty() {
                format!("component {name} {{}}")
            } else {
                format!("component {name} {{\n{}\n}}", lines.join("\n"))
            };
            (block, None)
        }
        Target::Prop { component, prop } => {
            let field = components.get(component)?.props.get(prop)?;
            let ty = Ty::from(&field.ty);
            let kind = if field.required {
                "Required"
            } else {
                "Optional"
            };
            (
                with_definition(format!("{prop}: {ty}"), &ty, manifest),
                Some(format!("{kind} prop of `{component}`.")),
            )
        }
        Target::Event { component, event } => {
            let declared = components.get(component)?.events.get(event)?;
            let line = event_line(event, declared.payload.as_ref());
            match &declared.payload {
                Some(payload) => (
                    with_definition(line, &Ty::from(payload), manifest),
                    Some(format!("Event of `{component}`.")),
                ),
                None => (
                    line,
                    Some(format!("Event of `{component}`, with no payload.")),
                ),
            }
        }
        Target::Scope(name) => {
            let ty = Ty::from(components.get(model.component)?.scope.get(name)?);
            (
                with_definition(format!("{name}: {ty}"), &ty, manifest),
                Some(format!("In the scope of `{}`.", model.component)),
            )
        }
        Target::Command(name) => {
            let command = components.get(model.component)?.commands.get(name)?;
            let parameters: Vec<String> = command
                .parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, Ty::from(&parameter.ty)))
                .collect();
            (
                format!("{name}({})", parameters.join(", ")),
                Some(format!("Command of `{}`.", model.component)),
            )
        }
    })
}

fn event_line(event: &str, payload: Option<&mesh_manifest::Type>) -> String {
    match payload {
        Some(payload) => format!("on.{event}: {}", Ty::from(payload)),
        None => format!("on.{event}"),
    }
}

fn typed_block(ty: &Ty, manifest: &Manifest) -> String {
    with_definition(ty.to_string(), ty, manifest)
}

/// `block`, and when `ty` is a named type (or an optional or list of
/// one), a second line with that type's definition. Only the outermost
/// named type is expanded, once.
fn with_definition(block: String, ty: &Ty, manifest: &Manifest) -> String {
    let mut inner = ty;
    while let Ty::Optional(next) | Ty::List(next) = inner {
        inner = next;
    }
    match inner {
        Ty::Named(name) => match manifest.types().get(name) {
            Some(definition) => format!("{block}\ntype {name} = {}", Ty::from(definition)),
            None => block,
        },
        _ => block,
    }
}

/// Where the name at `offset` is declared in the manifest, if it
/// resolved.
pub(super) fn definition(
    facts: Facts<'_>,
    model: Model<'_>,
    manifest_uri: &str,
    manifest_text: Text<'_>,
    offset: usize,
) -> Option<Location> {
    let resolution = facts.resolution_at(offset)?;
    let span = model
        .manifest
        .span_of(declaration_of(&resolution.target, model.component))?;
    Some(Location {
        uri: Uri::from_str(manifest_uri).ok()?,
        range: manifest_text.range(span),
    })
}

/// The manifest declaration `target` names, in the template of
/// `component`.
pub(super) fn declaration_of<'a>(target: &'a Target, component: &'a str) -> Declaration<'a> {
    match target {
        Target::Component(name) => Declaration::Component(name),
        Target::Prop { component, prop } => Declaration::Prop { component, prop },
        Target::Event { component, event } => Declaration::Event { component, event },
        Target::Scope(name) => Declaration::Scope { component, name },
        Target::Command(command) => Declaration::Command { component, command },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use mesh_compiler::{ColumnUnit, CompileOptions, SourceMap};

    const MANIFEST: &str = include_str!("../../../../examples/components.json");
    const PAGE: &str = include_str!("../../../../examples/users-page.mprx");

    fn hover_text(source: &str, needle: &str, nth_char: usize, markdown: bool) -> Option<String> {
        let manifest = mesh_manifest::load(MANIFEST).unwrap();
        let template = manifest.template("users-page").unwrap();
        let result = mesh_compiler::compile_with(source, &CompileOptions::with_template(template));
        let analysis = result.analysis.unwrap();
        let map = SourceMap::new(source);
        let text = Text {
            source,
            map: &map,
            unit: ColumnUnit::Utf16,
        };
        let offset = source.find(needle).unwrap() + nth_char;
        let model = Model {
            component: "users-page",
            manifest: &manifest,
        };
        hover(Facts::Canonical(&analysis), model, text, offset, markdown).map(|hover| {
            match hover.contents {
                HoverContents::Markup(markup) => markup.value,
                _ => panic!("hovers are markup"),
            }
        })
    }

    #[test]
    fn a_component_lists_its_props_and_events() {
        assert_eq!(
            hover_text(PAGE, "avatar", 0, false).unwrap(),
            "component avatar {\n  alt: string\n  size?: string\n  src: string?\n}"
        );
        assert_eq!(
            hover_text(PAGE, "button", 0, false).unwrap(),
            "component button {\n  disabled?: boolean\n  on.click: any\n}"
        );
        assert_eq!(
            hover_text(PAGE, "text", 0, false).unwrap(),
            "component text {}"
        );
    }

    #[test]
    fn a_prop_says_whether_it_is_required() {
        assert_eq!(
            hover_text(PAGE, "alt=", 0, false).unwrap(),
            "alt: string\n\nRequired prop of `avatar`."
        );
        assert_eq!(
            hover_text(PAGE, "size=", 0, false).unwrap(),
            "size: string\n\nOptional prop of `avatar`."
        );
    }

    #[test]
    fn an_event_shows_its_payload() {
        assert_eq!(
            hover_text(PAGE, "click", 0, false).unwrap(),
            "on.click: any\n\nEvent of `button`."
        );
    }

    #[test]
    fn a_scope_name_shows_its_type_and_expands_a_named_type_once() {
        assert_eq!(
            hover_text(PAGE, "compact ?", 0, false).unwrap(),
            "compact: boolean\n\nIn the scope of `users-page`."
        );
        assert_eq!(
            hover_text(PAGE, "user.name", 0, false).unwrap(),
            "user: User\ntype User = { active: boolean, avatar?: string, name: string }\n\nIn the scope of `users-page`."
        );
    }

    #[test]
    fn a_command_shows_its_parameters() {
        assert_eq!(
            hover_text(PAGE, "selectUser", 0, false).unwrap(),
            "selectUser(user: User)\n\nCommand of `users-page`."
        );
    }

    #[test]
    fn a_member_shows_its_type() {
        assert_eq!(hover_text(PAGE, "user.name", 6, false).unwrap(), "string");
        assert_eq!(
            hover_text(PAGE, "user.avatar", 6, false).unwrap(),
            "string?"
        );
    }

    #[test]
    fn markdown_or_plain_text_as_the_client_says() {
        assert_eq!(
            hover_text(PAGE, "user.name", 6, true).unwrap(),
            "```\nstring\n```"
        );
        assert_eq!(
            hover_text(PAGE, "compact ?", 0, true).unwrap(),
            "```\ncompact: boolean\n```\n\nIn the scope of `users-page`."
        );
    }

    #[test]
    fn text_has_no_hover() {
        assert_eq!(hover_text(PAGE, "Select<", 1, false), None);
    }
}
