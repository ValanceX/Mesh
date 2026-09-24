//! Presentation (outline, "Architecture and invariants", layer 3): the
//! compiler's diagnostics and suggestions as LSP values.
//!
//! Every position goes through [`SourceMap`] (invariant I4): this module
//! does no position arithmetic of its own. Every diagnostic maps to
//! exactly one LSP diagnostic, in order (invariant I3).

use lsp_types::{CodeDescription, DiagnosticSeverity, NumberOrString, Position, Range, Uri};
use mesh_compiler::{ColumnUnit, LineColumn, SourceMap};
use mesh_syntax::{Diagnostic, Severity, Span};
use std::str::FromStr;

/// Where each code's entry in the diagnostics reference is. GitHub makes
/// each `` ### `code` `` heading an anchor named after the code.
const REFERENCE: &str = "https://github.com/ValanceX/Mesh/blob/main/docs/manual/diagnostics.md";

/// A source text and its map, with the column unit the session
/// negotiated: everything needed to turn a byte offset into an LSP
/// position and back.
#[derive(Clone, Copy)]
pub(crate) struct Text<'a> {
    pub(crate) source: &'a str,
    pub(crate) map: &'a SourceMap,
    pub(crate) unit: ColumnUnit,
}

impl Text<'_> {
    pub(crate) fn position(&self, byte: usize) -> Position {
        let at = self.map.line_column(self.source, byte, self.unit);
        Position {
            line: saturate(at.line),
            character: saturate(at.column),
        }
    }

    pub(crate) fn range(&self, span: Span) -> Range {
        Range {
            start: self.position(span.start_byte),
            end: self.position(span.end_byte),
        }
    }

    /// The byte offset of `position`, clamped into the text.
    pub(crate) fn offset(&self, position: Position) -> usize {
        let at = LineColumn {
            line: widen(position.line),
            column: widen(position.character),
        };
        self.map.offset(self.source, at, self.unit)
    }

    /// The byte span `range` covers, with its ends in order.
    pub(crate) fn span(&self, range: Range) -> Span {
        let (a, b) = (self.offset(range.start), self.offset(range.end));
        Span {
            start_byte: a.min(b),
            end_byte: a.max(b),
        }
    }

    /// `diagnostic` as an LSP diagnostic.
    pub(crate) fn diagnostic(&self, diagnostic: &Diagnostic) -> lsp_types::Diagnostic {
        let code = diagnostic.code.as_str();
        lsp_types::Diagnostic {
            range: self.range(diagnostic.span),
            severity: Some(match diagnostic.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                // `Severity` is non-exhaustive; nothing else exists today.
                _ => DiagnosticSeverity::INFORMATION,
            }),
            code: Some(NumberOrString::String(code.to_string())),
            code_description: Uri::from_str(&format!("{REFERENCE}#{code}"))
                .ok()
                .map(|href| CodeDescription { href }),
            source: Some("mesh".to_string()),
            message: diagnostic.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Every one of `diagnostics`, in order.
    pub(crate) fn diagnostics(&self, diagnostics: &[Diagnostic]) -> Vec<lsp_types::Diagnostic> {
        diagnostics
            .iter()
            .map(|diagnostic| self.diagnostic(diagnostic))
            .collect()
    }
}

/// The source map's unit for an LSP `positionEncoding`. `utf-32` counts
/// Unicode scalar values, which is what `ColumnUnit::Char` counts.
pub(crate) fn unit(encoding: &str) -> ColumnUnit {
    match encoding {
        "utf-8" => ColumnUnit::Utf8,
        "utf-32" => ColumnUnit::Char,
        _ => ColumnUnit::Utf16,
    }
}

fn saturate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn widen(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use mesh_syntax::DiagnosticCode;

    fn diagnostic(severity: Severity, span: (usize, usize)) -> Diagnostic {
        Diagnostic {
            severity,
            code: DiagnosticCode::UNKNOWN_REFERENCE,
            message: "unknown reference \"usr\"".to_string(),
            span: Span {
                start_byte: span.0,
                end_byte: span.1,
            },
            suggestions: Vec::new(),
        }
    }

    fn at(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn carries_severity_code_reference_and_source() {
        let source = "<a x={usr} />";
        let map = SourceMap::new(source);
        let text = Text {
            source,
            map: &map,
            unit: ColumnUnit::Utf16,
        };
        let error = text.diagnostic(&diagnostic(Severity::Error, (6, 9)));
        assert_eq!(error.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            error.code,
            Some(NumberOrString::String("unknown-reference".into()))
        );
        assert_eq!(
            error.code_description.map(|d| d.href.as_str().to_string()),
            Some(format!("{REFERENCE}#unknown-reference"))
        );
        assert_eq!(error.source.as_deref(), Some("mesh"));
        assert_eq!(error.message, "unknown reference \"usr\"");
        let warning = text.diagnostic(&diagnostic(Severity::Warning, (6, 9)));
        assert_eq!(warning.severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn ranges_count_the_negotiated_unit() {
        // A BOM, a CRLF line, and an emoji before the span on line 1.
        let source = "\u{feff}<a>\r\n😀 {usr}</a>";
        let map = SourceMap::new(source);
        let start = source.find("usr").unwrap();
        let span = (start, start + 3);
        let range = |unit| {
            Text {
                source,
                map: &map,
                unit,
            }
            .diagnostic(&diagnostic(Severity::Error, span))
            .range
        };
        assert_eq!(range(ColumnUnit::Utf16).start, at(1, 4));
        assert_eq!(range(ColumnUnit::Utf8).start, at(1, 6));
        assert_eq!(range(ColumnUnit::Char).start, at(1, 3));
        for unit in [ColumnUnit::Utf16, ColumnUnit::Utf8, ColumnUnit::Char] {
            let text = Text {
                source,
                map: &map,
                unit,
            };
            assert_eq!(
                text.span(range(unit)),
                Span {
                    start_byte: span.0,
                    end_byte: span.1
                }
            );
        }
    }

    #[test]
    fn a_zero_width_span_stays_zero_width() {
        let source = "<a x={1 +} />";
        let map = SourceMap::new(source);
        let text = Text {
            source,
            map: &map,
            unit: ColumnUnit::Utf16,
        };
        let range = text.range(Span {
            start_byte: 9,
            end_byte: 9,
        });
        assert_eq!(range.start, range.end);
    }

    #[test]
    fn keeps_every_diagnostic_in_order() {
        let source = "<a x={usr} y={usr} />";
        let map = SourceMap::new(source);
        let text = Text {
            source,
            map: &map,
            unit: ColumnUnit::Utf16,
        };
        let input = [
            diagnostic(Severity::Error, (14, 17)),
            diagnostic(Severity::Warning, (6, 9)),
            diagnostic(Severity::Error, (14, 17)),
        ];
        let output = text.diagnostics(&input);
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].range.start, at(0, 14));
        assert_eq!(output[1].range.start, at(0, 6));
        assert_eq!(output[2].range.start, at(0, 14));
    }

    #[test]
    fn positions_past_the_text_clamp() {
        let source = "ab\ncd";
        let map = SourceMap::new(source);
        let text = Text {
            source,
            map: &map,
            unit: ColumnUnit::Utf16,
        };
        assert_eq!(text.offset(at(0, 99)), 2);
        assert_eq!(text.offset(at(99, 0)), 5);
        assert_eq!(text.offset(at(u32::MAX, u32::MAX)), 5);
    }

    #[test]
    fn maps_encodings_to_units() {
        assert_eq!(unit("utf-8"), ColumnUnit::Utf8);
        assert_eq!(unit("utf-16"), ColumnUnit::Utf16);
        assert_eq!(unit("utf-32"), ColumnUnit::Char);
        assert_eq!(unit("anything"), ColumnUnit::Utf16);
    }
}
