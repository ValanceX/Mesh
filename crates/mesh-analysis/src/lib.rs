//! Semantic analysis for MPRX: checks a template's Semantic IR against the
//! component manifest it is the template of.
//!
//! [`analyze`] resolves every name in the IR against the manifest: each
//! element's tag to a component, its attributes to that component's props
//! and its `on.` bindings to its events, and every reference and command to
//! the template's own scope and commands. It also checks that required
//! props are supplied and that commands and `$event` appear only where
//! they may.
//!
//! It then types every expression ([`Ty`]) by the language's typing
//! rules, with [`is_assignable`] and [`join`] as the only ways of
//! comparing types.
//!
//! Analysis never changes the IR. Its results are kept beside it, in the
//! returned [`Analysis`]: what each name resolved to, each expression's
//! type, and a [`Fact`] for each problem found. Facts are data, not
//! prose; turning them into diagnostics (codes, messages and suggestions)
//! is `mesh-compiler`'s job.

use mesh_manifest::Template;
use mesh_syntax::{BinaryOperator, Span, UnaryOperator};

mod check;
mod relation;
mod types;

pub use relation::{is_assignable, join};
pub use types::{FieldTy, Ty};

/// Analyzes `ir` as the template of `template`'s component.
pub fn analyze(ir: &mesh_semantic::Element, template: Template<'_>) -> Analysis {
    check::analyze(ir, template)
}

/// Analyzes one expression in the template of `template`'s component, as
/// the value of an attribute whose type nothing constrains: not as an
/// `on.` handler, and not as a handler command's argument. So a command
/// in it is `CommandOutsideHandler`, and `$event` is
/// `EventValueOutsideHandler`.
///
/// It runs the same walk as [`analyze`], so a reference, member access or
/// operator gets exactly the resolution and type it would get in a
/// template. An editor uses it for an expression it recovered from a file
/// with syntax errors, where there is no element to analyze.
pub fn analyze_expression(
    expression: &mesh_semantic::Expression,
    template: Template<'_>,
) -> Analysis {
    check::analyze_expression(expression, template)
}

/// The results of analyzing one template: kept beside the IR, never
/// written into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    facts: Vec<Fact>,
    resolutions: Vec<Resolution>,
    types: Vec<Typed>,
}

impl Analysis {
    /// Every problem found, in source order: sorted by the start of each
    /// fact's span, and in the order they were found where two start at
    /// the same byte.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// What each name that resolved refers to, in the order analysis met
    /// them. A name that didn't resolve has a [`Fact`] instead.
    pub fn resolutions(&self) -> &[Resolution] {
        &self.resolutions
    }

    /// The type of every expression that has one, in the order analysis
    /// typed them (operands before the expression they're in). An
    /// expression has no type when a fact about it, or about a part of it,
    /// was reported instead, with one exception: an operator whose result
    /// type is fixed (`!`, `-`, arithmetic, comparisons, `&&`, `||`, `==`
    /// and `!=`) keeps that type whatever its operands are, since it can't
    /// cause a second, spurious diagnostic.
    pub fn types(&self) -> &[Typed] {
        &self.types
    }

    /// The type of the expression whose span is exactly `span`, if it has
    /// one.
    pub fn type_at(&self, span: Span) -> Option<&Ty> {
        self.types
            .iter()
            .find(|typed| typed.span == span)
            .map(|typed| &typed.ty)
    }

    /// The name at byte `offset` and what it resolved to, if a resolved
    /// name contains it. See [`Analysis::typed_at`] for what "at" means.
    pub fn resolution_at(&self, offset: usize) -> Option<&Resolution> {
        innermost(&self.resolutions, offset, |resolution| resolution.span)
    }

    /// The innermost expression with a type that contains byte `offset`.
    ///
    /// A span contains an offset from its start to its end, both
    /// included, so a cursor just after a name still finds it. Where
    /// several spans contain the offset, the shortest wins. Two different
    /// spans of the same length can only both contain it where one ends
    /// and the other starts, and then the one that starts there wins.
    pub fn typed_at(&self, offset: usize) -> Option<&Typed> {
        innermost(&self.types, offset, |typed| typed.span)
    }

    /// Every fact whose span contains byte `offset`, in [`Analysis::facts`]
    /// order. Containment is as for [`Analysis::typed_at`].
    pub fn facts_at(&self, offset: usize) -> impl Iterator<Item = &Fact> {
        self.facts
            .iter()
            .filter(move |fact| contains(fact.span(), offset))
    }
}

/// Whether `span` contains `offset`, both ends included.
fn contains(span: Span, offset: usize) -> bool {
    span.start_byte <= offset && offset <= span.end_byte
}

/// The item of `items` whose span contains `offset` and is shortest,
/// preferring the later start between spans of equal length.
fn innermost<T>(items: &[T], offset: usize, span: impl Fn(&T) -> Span) -> Option<&T> {
    items
        .iter()
        .filter(|item| contains(span(item), offset))
        .min_by_key(|item| {
            let span = span(item);
            (
                span.end_byte - span.start_byte,
                std::cmp::Reverse(span.start_byte),
            )
        })
}

/// An expression's span, and its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub span: Span,
    pub ty: Ty,
}

/// Why a type was expected where a [`Fact::TypeMismatch`] was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expectation {
    /// An operator's operand.
    Operand(Operator),
    /// The condition of `c ? a : b`.
    Condition,
    /// A prop's value.
    Prop { component: String, prop: String },
    /// A command's argument.
    Argument { command: String, parameter: String },
    /// A field's value in an object literal.
    Field { field: String },
    /// An element of an array literal where a list is expected.
    Element,
}

/// A unary or binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Unary(UnaryOperator),
    Binary(BinaryOperator),
}

/// Where two types needed a common type ([`join`]) and had none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combination {
    /// The two branches of `c ? a : b`.
    Branches,
    /// An array literal's elements.
    Elements,
    /// The operands of `==` or `!=`.
    Equality(BinaryOperator),
}

/// A name in the template, and the manifest declaration it refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The name as written: a tag name, attribute name, event name,
    /// reference or command name.
    pub span: Span,
    pub target: Target,
}

/// The manifest declaration a name resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A component, by tag name.
    Component(String),
    /// A prop of a component.
    Prop { component: String, prop: String },
    /// An event of a component.
    Event { component: String, event: String },
    /// A name in the template's scope.
    Scope(String),
    /// A command of the template's component.
    Command(String),
}

/// A problem analysis found, with the context a diagnostic needs.
///
/// Facts hold no prose and no suggestions: `candidates` lists the names
/// that *were* available, and choosing a close one is the diagnostic
/// layer's job. Candidates are in the manifest's (sorted) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact {
    /// An element's tag names no component. Span: the tag name.
    UnknownComponent {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// An attribute isn't a prop of the element's component. Span: the
    /// attribute name.
    UnknownProp {
        component: String,
        prop: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// An element omits a prop its component declares required. Span: the
    /// element's tag name.
    MissingRequiredProp {
        component: String,
        prop: String,
        span: Span,
    },
    /// An `on.` binding names an event the component doesn't have. Span:
    /// the event name.
    UnknownEvent {
        component: String,
        event: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A reference isn't in the template's scope. Span: the reference.
    UnknownReference {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A command the template's component doesn't declare. Span: the
    /// command name.
    UnknownCommand {
        command: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A command invoked with the wrong number of arguments. Span: the
    /// whole invocation.
    CommandArityMismatch {
        command: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A command invoked anywhere but as the whole handler of an `on.`
    /// binding. Span: the whole invocation.
    CommandOutsideHandler { command: String, span: Span },
    /// An `on.` handler that isn't a command invocation. Span: the
    /// handler expression.
    HandlerNotCommand { event: String, span: Span },
    /// `$event` outside the arguments of an `on.` handler's command.
    EventValueOutsideHandler { span: Span },
    /// `$event` in the handler of an event that declares no payload.
    EventHasNoPayload {
        component: String,
        event: String,
        span: Span,
    },
    /// A `$` name other than `$event`. `name` has no `$`.
    UnknownSpecialValue {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A member access whose object has no such member: a record without
    /// that field, or a type with no members at all. Span: the property
    /// name. `candidates` are the record's fields, if it is one.
    UnknownMember {
        object: Ty,
        property: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A member access on a value that may be absent. Span: the object.
    PossiblyAbsentAccess {
        object: Ty,
        property: String,
        span: Span,
    },
    /// A value whose type isn't assignable to the one expected. Span: the
    /// value. `possibly_absent` says why when that is the reason: `actual`
    /// is optional, `expected` isn't, and `actual` would fit if present.
    TypeMismatch {
        expectation: Expectation,
        expected: Ty,
        actual: Ty,
        possibly_absent: bool,
        span: Span,
    },
    /// Two types that needed a common type and have none. Span: the
    /// conditional or the equality, or the first array element that
    /// doesn't fit the ones before it (`left` is their common type).
    NoCommonType {
        combination: Combination,
        left: Ty,
        right: Ty,
        span: Span,
    },
    /// An object literal repeats a key. Span: an earlier occurrence of
    /// the key, which the last one shadows; `last` is the last one's key.
    DuplicateObjectKey { key: String, span: Span, last: Span },
    /// An object literal, where the record type `record` is expected, has
    /// a key that isn't one of its fields. Span: the key.
    UnknownField {
        record: Ty,
        field: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// An object literal, where the record type `record` is expected,
    /// lacks a field it declares required. Span: the object literal.
    MissingRequiredField {
        record: Ty,
        field: String,
        span: Span,
    },
}

impl Fact {
    /// The source span the fact is about.
    pub fn span(&self) -> Span {
        match self {
            Fact::UnknownComponent { span, .. }
            | Fact::UnknownProp { span, .. }
            | Fact::MissingRequiredProp { span, .. }
            | Fact::UnknownEvent { span, .. }
            | Fact::UnknownReference { span, .. }
            | Fact::UnknownCommand { span, .. }
            | Fact::CommandArityMismatch { span, .. }
            | Fact::CommandOutsideHandler { span, .. }
            | Fact::HandlerNotCommand { span, .. }
            | Fact::EventValueOutsideHandler { span }
            | Fact::EventHasNoPayload { span, .. }
            | Fact::UnknownSpecialValue { span, .. }
            | Fact::UnknownMember { span, .. }
            | Fact::PossiblyAbsentAccess { span, .. }
            | Fact::TypeMismatch { span, .. }
            | Fact::NoCommonType { span, .. }
            | Fact::DuplicateObjectKey { span, .. }
            | Fact::UnknownField { span, .. }
            | Fact::MissingRequiredField { span, .. } => *span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start_byte: usize, end_byte: usize) -> Span {
        Span {
            start_byte,
            end_byte,
        }
    }

    /// Analysis lists operands before the expressions around them, so
    /// the example-based tests can't tell "innermost" from "first". These
    /// list the outer span first.
    #[test]
    fn the_shortest_containing_span_wins_whatever_the_order() {
        let spans = [span(0, 10), span(2, 8), span(3, 5), span(20, 21)];
        assert_eq!(innermost(&spans, 4, |s| *s), Some(&span(3, 5)));
        assert_eq!(innermost(&spans, 1, |s| *s), Some(&span(0, 10)));
        assert_eq!(innermost(&spans, 15, |s| *s), None);
    }

    #[test]
    fn both_ends_are_included_and_a_tie_goes_to_the_later_start() {
        let spans = [span(0, 3), span(3, 6)];
        assert_eq!(innermost(&spans, 0, |s| *s), Some(&span(0, 3)));
        assert_eq!(innermost(&spans, 3, |s| *s), Some(&span(3, 6)));
        assert_eq!(innermost(&spans, 6, |s| *s), Some(&span(3, 6)));
    }
}
