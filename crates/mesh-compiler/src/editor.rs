//! **Editor-only** answers about files with syntax errors.
//!
//! [`crate::compile_with`] gives a file with a syntax error its syntax
//! errors and nothing else: no IR, no analysis. That is the right answer,
//! and it stays the only answer about what a file *means* (outline
//! invariant I2). But while someone is typing, a file is broken most of
//! the time, and an editor still wants hover and go-to-definition on the
//! parts that parse.
//!
//! [`recover`] gives it those parts, analyzed by the same analysis code
//! as a compile, against the same template. Its [`Recovery`] answers only
//! "what name, or what typed expression, is at this offset". It has no
//! facts and no diagnostics, on purpose, so nothing recovered can be
//! reported as a mistake, cached as the compiler's verdict, or compared
//! with it. `compile`, `compile_with` and `mesh check` never call it.
//!
//! The rules for what is recovered are in
//! `docs/superpowers/specs/2026-09-24-mesh-v0.3-editor-recovery.md`.
//!
//! [`candidates`] answers the other question an editor asks while the
//! file is broken: what may be written here? Its answers come only from
//! the manifest's declarations, offered only where analysis accepts them.

use mesh_analysis::{Analysis, Resolution, Typed};
use mesh_manifest::Template;
use mesh_syntax::Span;

/// What the parts of a broken file that parse mean: for each part, the
/// resolutions and types analysis gave it.
///
/// It has no facts and no diagnostics, so it can't report anything:
///
/// ```compile_fail
/// fn report(recovery: &mesh_compiler::editor::Recovery) {
///     let _ = recovery.facts();
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recovery {
    /// One analysis per recovered part, in source order. Their facts are
    /// never read.
    parts: Vec<Analysis>,
}

impl Recovery {
    /// The resolved name at byte `offset`, by [`Analysis::resolution_at`]'s
    /// rule across every part: the innermost wins.
    pub fn resolution_at(&self, offset: usize) -> Option<&Resolution> {
        innermost(
            self.parts
                .iter()
                .filter_map(|part| part.resolution_at(offset)),
            |resolution| resolution.span,
        )
    }

    /// The innermost typed expression at byte `offset`, by
    /// [`Analysis::typed_at`]'s rule across every part.
    pub fn typed_at(&self, offset: usize) -> Option<&Typed> {
        innermost(
            self.parts.iter().filter_map(|part| part.typed_at(offset)),
            |typed| typed.span,
        )
    }

    /// How many parts were recovered.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether nothing was recovered.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

/// The shortest span, the later start on a tie: `Analysis`'s own rule.
fn innermost<'a, T>(
    items: impl Iterator<Item = &'a T>,
    span: impl Fn(&T) -> Span,
) -> Option<&'a T> {
    items.min_by_key(|item| {
        let span = span(item);
        (
            span.end_byte - span.start_byte,
            std::cmp::Reverse(span.start_byte),
        )
    })
}

/// Recovers what parses of `source` and analyzes it as the template of
/// `template` (see the module documentation). Every element part is
/// analyzed as [`crate::compile_with`] would analyze it inside the whole
/// file, and every expression part as an unconstrained value. Lowering
/// diagnostics and facts are dropped here.
pub fn recover(source: &str, template: Template<'_>) -> Recovery {
    let recovered = mesh_parser::recover(source);
    let mut parts: Vec<(usize, Analysis)> = Vec::new();
    for element in &recovered.elements {
        if let Some(ir) = mesh_semantic::lower(element).ir {
            parts.push((ir.span.start_byte, mesh_analysis::analyze(&ir, template)));
        }
    }
    for part in &recovered.expressions {
        let expression = mesh_semantic::lower_expression(&part.expression);
        parts.push((
            part.span.start_byte,
            mesh_analysis::analyze_expression(&expression, template),
        ));
    }
    parts.sort_by_key(|(start, _)| *start);
    Recovery {
        parts: parts.into_iter().map(|(_, analysis)| analysis).collect(),
    }
}

/// What a completion candidate names.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateKind {
    Component,
    Prop,
    Event,
    ScopeName,
    Member,
    Command,
}

/// A name that may be written at the cursor (outline D4), derived from a
/// declaration in the manifest.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The text to write, such as `alt`, `on.click` or `user-card`.
    pub label: String,
    pub kind: CandidateKind,
    /// Its declared type, or a command's signature, in the notation
    /// diagnostics use.
    pub detail: Option<String>,
    /// Whether a prop or record field is required.
    pub required: bool,
    /// The partial name the label replaces.
    pub replace: Span,
}

/// The names that may be written at byte `offset` of `source`, checked as
/// the template of `template` (outline D4). `analysis` is the canonical
/// compile's analysis of the same `source`, if it has one.
///
/// Every candidate comes from a declaration in the manifest, and each
/// kind is offered only where analysis accepts it: components as tag
/// names, a component's props and `on.` events in its opening tag,
/// commands as the whole of an `on.` handler, scope names where any other
/// expression may start, and after `.` exactly the members
/// [`mesh_analysis::members`] gives the object's type. The object's type
/// is the canonical analysis's, or, for a file with no analysis (a syntax
/// error), [`mesh_analysis::analyze_expression`]'s. No type, no members.
///
/// The list is deterministic and has no two candidates with the same
/// label and kind. Required props come before optional ones; otherwise
/// everything is in name order, the manifest's.
pub fn candidates(
    source: &str,
    offset: usize,
    template: Template<'_>,
    analysis: Option<&Analysis>,
) -> Vec<Candidate> {
    use mesh_analysis::{members, read_field, Ty};
    use mesh_parser::Context;

    let Some(completion) = mesh_parser::context_at(source, offset) else {
        return Vec::new();
    };
    let replace = completion.replace;
    let candidate = |label: String, kind, detail: Option<String>, required| Candidate {
        label,
        kind,
        detail,
        required,
        replace,
    };
    let manifest = template.manifest();
    let mut out: Vec<Candidate> = match completion.context {
        Context::TagName => manifest
            .components()
            .keys()
            .map(|name| candidate(name.clone(), CandidateKind::Component, None, false))
            .collect(),
        Context::AttributeName { tag } => {
            let Some(component) = manifest.components().get(&tag) else {
                return Vec::new();
            };
            let props = |required: bool| {
                component
                    .props
                    .iter()
                    .filter(move |(_, field)| field.required == required)
                    .map(move |(name, field)| {
                        candidate(
                            name.clone(),
                            CandidateKind::Prop,
                            Some(Ty::from(&field.ty).to_string()),
                            required,
                        )
                    })
            };
            let events = component.events.iter().map(|(name, event)| {
                candidate(
                    format!("on.{name}"),
                    CandidateKind::Event,
                    event.payload.as_ref().map(|ty| Ty::from(ty).to_string()),
                    false,
                )
            });
            props(true).chain(props(false)).chain(events).collect()
        }
        Context::EventName { tag } => {
            let Some(component) = manifest.components().get(&tag) else {
                return Vec::new();
            };
            component
                .events
                .iter()
                .map(|(name, event)| {
                    candidate(
                        name.clone(),
                        CandidateKind::Event,
                        event.payload.as_ref().map(|ty| Ty::from(ty).to_string()),
                        false,
                    )
                })
                .collect()
        }
        Context::Handler => template
            .component()
            .commands
            .iter()
            .map(|(name, command)| {
                let parameters: Vec<String> = command
                    .parameters
                    .iter()
                    .map(|parameter| format!("{}: {}", parameter.name, Ty::from(&parameter.ty)))
                    .collect();
                candidate(
                    name.clone(),
                    CandidateKind::Command,
                    Some(format!("{name}({})", parameters.join(", "))),
                    false,
                )
            })
            .collect(),
        Context::Value => template
            .component()
            .scope
            .iter()
            .map(|(name, ty)| {
                candidate(
                    name.clone(),
                    CandidateKind::ScopeName,
                    Some(Ty::from(ty).to_string()),
                    false,
                )
            })
            .collect(),
        Context::Member {
            object,
            object_span,
        } => {
            let ty = match analysis {
                Some(analysis) => analysis.type_at(object_span).cloned(),
                None => {
                    let object = mesh_semantic::lower_expression(&object);
                    mesh_analysis::analyze_expression(&object, template)
                        .type_at(object_span)
                        .cloned()
                }
            };
            ty.and_then(|ty| members(manifest, &ty))
                .into_iter()
                .flatten()
                .map(|(name, field)| {
                    let read = read_field(manifest, &field);
                    candidate(
                        name,
                        CandidateKind::Member,
                        Some(read.to_string()),
                        field.required,
                    )
                })
                .collect()
        }
        _ => Vec::new(),
    };
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|candidate| seen.insert((candidate.label.clone(), candidate.kind)));
    out
}
