//! Formatting (outline: no semantics of its own): Format Document, Format
//! Selection, and the indentation of the line after Enter, all from
//! `mesh-format`, on the document's current text. They need no compile, so
//! they are answered at once, never held.

use super::Server;
use crate::convert::Text;
use lsp_types::{
    DocumentFormattingParams, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    FormattingOptions, Position, Range, TextEdit,
};
use mesh_compiler::SourceMap;
use mesh_format::{Edit, Options};
use mesh_syntax::Span;

fn options(from: &FormattingOptions) -> Options {
    Options {
        tab_size: usize::try_from(from.tab_size).unwrap_or(2).clamp(1, 16),
        use_tabs: !from.insert_spaces,
    }
}

impl Server {
    /// Runs `with` on the text of `document`, with the map that turns offsets into positions.
    fn with_text<T>(&self, uri: &str, with: impl FnOnce(Text<'_>) -> T) -> Option<T> {
        let document = self.documents.get(uri)?;
        let source = &*document.text;
        let map = SourceMap::new(source);
        Some(with(Text {
            source,
            map: &map,
            unit: self.unit,
        }))
    }

    fn lsp_edits(text: Text<'_>, edits: Vec<Edit>) -> Vec<TextEdit> {
        edits
            .into_iter()
            .map(|edit| TextEdit {
                range: text.range(Span {
                    start_byte: edit.start,
                    end_byte: edit.end,
                }),
                new_text: edit.text,
            })
            .collect()
    }

    /// Format Document. `None` (no edits) if the text has syntax errors.
    pub(super) fn format_document(
        &self,
        params: &DocumentFormattingParams,
    ) -> Option<Vec<TextEdit>> {
        let wanted = options(&params.options);
        self.with_text(params.text_document.uri.as_str(), |text| {
            mesh_format::format_edits(text.source, &wanted)
                .ok()
                .map(|edits| Self::lsp_edits(text, edits))
        })?
    }

    /// Format Selection.
    pub(super) fn format_range(
        &self,
        params: &DocumentRangeFormattingParams,
    ) -> Option<Vec<TextEdit>> {
        let wanted = options(&params.options);
        self.with_text(params.text_document.uri.as_str(), |text| {
            let span = text.span(params.range);
            mesh_format::format_range_edits(text.source, span.start_byte, span.end_byte, &wanted)
                .ok()
                .map(|edits| Self::lsp_edits(text, edits))
        })?
    }

    /// The indentation of the line the cursor is on, after Enter: it replaces
    /// whatever whitespace the line starts with. Works while the text does not parse.
    pub(super) fn format_on_type(
        &self,
        params: &DocumentOnTypeFormattingParams,
    ) -> Option<Vec<TextEdit>> {
        let wanted = options(&params.options);
        let position = params.text_document_position.position;
        self.with_text(
            params.text_document_position.text_document.uri.as_str(),
            |text| {
                let line = Position {
                    line: position.line,
                    character: 0,
                };
                let start = text.offset(line);
                let rest = &text.source[start..];
                let blank = rest.len() - rest.trim_start_matches([' ', '\t']).len();
                let indent = mesh_format::indent_for_line(text.source, start, &wanted);
                if text.source[start..start + blank] == indent {
                    return Vec::new();
                }
                vec![TextEdit {
                    range: Range {
                        start: line,
                        end: text.position(start + blank),
                    },
                    new_text: indent,
                }]
            },
        )
    }
}
