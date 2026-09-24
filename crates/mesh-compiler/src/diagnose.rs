//! The diagnostic layer: turns analysis [`Fact`]s into [`Diagnostic`]s.
//!
//! This is the only place that chooses a fact's code, writes its message
//! and computes suggestions. Analysis reports what it found and which
//! names were available; this layer decides how to say it.

use mesh_analysis::{Combination, Expectation, Fact, Operator};
use mesh_syntax::{
    BinaryOperator, Diagnostic, DiagnosticCode, Severity, Span, Suggestion, UnaryOperator,
};

/// The diagnostic for `fact`. Every analysis fact is an error.
pub(crate) fn diagnostic(fact: &Fact) -> Diagnostic {
    let (code, message, suggestions) = match fact {
        Fact::UnknownComponent {
            name,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_COMPONENT,
            format!("unknown component {name:?}: the manifest doesn't declare it"),
            suggest(name, candidates, *span, ""),
        ),
        Fact::UnknownProp {
            component,
            prop,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_PROP,
            format!("component {component:?} has no prop {prop:?}"),
            suggest(prop, candidates, *span, ""),
        ),
        Fact::MissingRequiredProp {
            component,
            prop,
            ..
        } => (
            DiagnosticCode::MISSING_REQUIRED_PROP,
            format!("component {component:?} requires the prop {prop:?}"),
            Vec::new(),
        ),
        Fact::UnknownEvent {
            component,
            event,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_EVENT,
            format!("component {component:?} has no event {event:?}"),
            suggest(event, candidates, *span, ""),
        ),
        Fact::UnknownReference {
            name,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_REFERENCE,
            format!("unknown reference {name:?}: it isn't in the template's scope"),
            suggest(name, candidates, *span, ""),
        ),
        Fact::UnknownCommand {
            command,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_COMMAND,
            format!("unknown command {command:?}: the template's component doesn't declare it"),
            suggest(command, candidates, *span, ""),
        ),
        Fact::CommandArityMismatch {
            command,
            expected,
            found,
            ..
        } => (
            DiagnosticCode::COMMAND_ARITY_MISMATCH,
            format!(
                "command {command:?} takes {}, but {} given",
                count(*expected, "argument"),
                match found {
                    1 => "1 was".to_string(),
                    n => format!("{n} were"),
                }
            ),
            Vec::new(),
        ),
        Fact::CommandOutsideHandler { command, .. } => (
            DiagnosticCode::COMMAND_OUTSIDE_HANDLER,
            format!(
                "command {command:?} can only be invoked as the whole handler of an `on.` binding"
            ),
            Vec::new(),
        ),
        Fact::HandlerNotCommand { event, .. } => (
            DiagnosticCode::HANDLER_NOT_COMMAND,
            format!("the handler of `on.{event}` must be a command invocation, like `save()`"),
            Vec::new(),
        ),
        Fact::EventValueOutsideHandler { .. } => (
            DiagnosticCode::EVENT_VALUE_OUTSIDE_HANDLER,
            "`$event` can only be used in the arguments of an `on.` handler's command".to_string(),
            Vec::new(),
        ),
        Fact::EventHasNoPayload {
            component, event, ..
        } => (
            DiagnosticCode::EVENT_HAS_NO_PAYLOAD,
            format!(
                "event {event:?} of component {component:?} carries no value, so there is no `$event`"
            ),
            Vec::new(),
        ),
        Fact::UnknownSpecialValue {
            name,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_SPECIAL_VALUE,
            format!("unknown special value `${name}`: the only one is `$event`"),
            suggest(name, candidates, *span, "$"),
        ),
        Fact::UnknownMember {
            object,
            property,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_MEMBER,
            format!("{object} has no member {property:?}"),
            suggest(property, candidates, *span, ""),
        ),
        Fact::PossiblyAbsentAccess {
            object, property, ..
        } => (
            DiagnosticCode::POSSIBLY_ABSENT_ACCESS,
            format!(
                "this value may be absent (its type is {object}), so its member {property:?} can't be read; MPRX has no optional chaining"
            ),
            Vec::new(),
        ),
        Fact::TypeMismatch {
            expectation,
            expected,
            actual,
            possibly_absent,
            ..
        } => (
            DiagnosticCode::TYPE_MISMATCH,
            format!(
                "{} needs {expected}, found {actual}{}",
                expecting(expectation),
                if *possibly_absent {
                    ", which may be absent"
                } else {
                    ""
                }
            ),
            Vec::new(),
        ),
        Fact::NoCommonType {
            combination,
            left,
            right,
            ..
        } => (
            DiagnosticCode::NO_COMMON_TYPE,
            match combination {
                Combination::Branches => {
                    format!("the branches have no common type: {left} and {right}")
                }
                Combination::Elements => format!(
                    "this element doesn't fit with the ones before it: {left} and {right} have no common type"
                ),
                Combination::Equality(operator) => format!(
                    "`{}` compares {left} with {right}, which have no common type",
                    binary(*operator)
                ),
            },
            Vec::new(),
        ),
        Fact::DuplicateObjectKey { key, .. } => (
            DiagnosticCode::DUPLICATE_OBJECT_KEY,
            format!("duplicate object key {key:?}: this occurrence is shadowed by a later one"),
            Vec::new(),
        ),
        Fact::UnknownField {
            record,
            field,
            span,
            candidates,
        } => (
            DiagnosticCode::UNKNOWN_FIELD,
            format!("{record} has no field {field:?}"),
            suggest(field, candidates, *span, ""),
        ),
        Fact::MissingRequiredField { record, field, .. } => (
            DiagnosticCode::MISSING_REQUIRED_FIELD,
            format!("the object is missing the field {field:?}, which {record} requires"),
            Vec::new(),
        ),
    };
    Diagnostic {
        severity: Severity::Error,
        code,
        message,
        span: fact.span(),
        suggestions,
    }
}

/// What expected a type, as the start of a sentence.
fn expecting(expectation: &Expectation) -> String {
    match expectation {
        Expectation::Operand(Operator::Unary(operator)) => {
            format!("the operand of `{}`", unary(*operator))
        }
        Expectation::Operand(Operator::Binary(operator)) => {
            format!("an operand of `{}`", binary(*operator))
        }
        Expectation::Condition => "the condition".to_string(),
        Expectation::Prop { component, prop } => {
            format!("the prop {prop:?} of component {component:?}")
        }
        Expectation::Argument { command, parameter } => {
            format!("the argument {parameter:?} of command {command:?}")
        }
        Expectation::Field { field } => format!("the field {field:?}"),
        Expectation::Element => "the list element".to_string(),
    }
}

fn unary(operator: UnaryOperator) -> &'static str {
    match operator {
        UnaryOperator::Not => "!",
        UnaryOperator::Negate => "-",
    }
}

fn binary(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Mul => "*",
        BinaryOperator::Div => "/",
        BinaryOperator::Mod => "%",
        BinaryOperator::Add => "+",
        BinaryOperator::Sub => "-",
        BinaryOperator::Lt => "<",
        BinaryOperator::Le => "<=",
        BinaryOperator::Gt => ">",
        BinaryOperator::Ge => ">=",
        BinaryOperator::Eq => "==",
        BinaryOperator::Ne => "!=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

/// `n` and `noun`, pluralized: `1 argument`, `0 arguments`.
fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// A did-you-mean suggestion: the candidate closest to `name`, if one is
/// close enough, as a replacement for `span` written with `prefix`.
fn suggest(name: &str, candidates: &[String], span: Span, prefix: &str) -> Vec<Suggestion> {
    closest(name, candidates)
        .map(|candidate| Suggestion {
            replacement: format!("{prefix}{candidate}"),
            span,
        })
        .into_iter()
        .collect()
}

/// The candidate with the smallest edit distance to `name`, if that
/// distance is at most a third of `name`'s length (and at least 1).
/// Ties go to the earliest candidate, so the choice is deterministic.
pub(crate) fn closest<'a>(name: &str, candidates: &'a [String]) -> Option<&'a str> {
    let limit = name.chars().count().max(3) / 3;
    candidates
        .iter()
        .map(|candidate| (edit_distance(name, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.as_str())
}

/// The edit distance between `a` and `b`, in `char`s: the fewest
/// insertions, deletions, substitutions and swaps of two adjacent
/// characters that turn one into the other (optimal string alignment).
/// A swap counts as one edit, so `nmae` is one edit from `name`.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    // d[i][j]: the distance between the first i chars of a and the first
    // j chars of b.
    let mut d: Vec<Vec<usize>> = (0..=a.len())
        .map(|i| (0..=b.len()).map(|j| if i == 0 { j } else { i }).collect())
        .collect();
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let substitution = d[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
            let mut best = substitution.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn measures_edit_distance_in_chars() {
        assert_eq!(edit_distance("usr", "user"), 1);
        assert_eq!(edit_distance("user", "user"), 0);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("größe", "grösse"), 2);
        assert_eq!(edit_distance("nmae", "name"), 1);
        assert_eq!(edit_distance("ab", "ba"), 1);
    }

    #[test]
    fn suggests_only_close_names() {
        let candidates = names(&["compact", "user"]);
        assert_eq!(closest("usr", &candidates), Some("user"));
        assert_eq!(closest("compcat", &candidates), Some("compact"));
        assert_eq!(closest("uesr", &candidates), Some("user"));
        assert_eq!(closest("layout", &candidates), None);
        assert_eq!(closest("u", &candidates), None);
        assert_eq!(closest("x", &names(&["y"])), Some("y"));
    }

    #[test]
    fn breaks_ties_by_candidate_order() {
        assert_eq!(closest("bat", &names(&["cat", "hat"])), Some("cat"));
        assert_eq!(closest("bat", &names(&["hat", "cat"])), Some("hat"));
    }
}
