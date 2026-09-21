//! This crate provides Mprx language support for the [tree-sitter] parsing library.
//!
//! Typically, you will use the [`LANGUAGE`] constant to add this language to a
//! tree-sitter [`Parser`], and then use the parser to parse some code:
//!
//! ```
//! let code = r#"<page title="Users" />"#;
//! let mut parser = tree_sitter::Parser::new();
//! let language = tree_sitter_mprx::LANGUAGE;
//! parser
//!     .set_language(&language.into())
//!     .expect("Error loading Mprx parser");
//! let tree = parser.parse(code, None).unwrap();
//! assert!(!tree.root_node().has_error());
//! ```
//!
//! [`Parser`]: https://docs.rs/tree-sitter/0.27.0/tree_sitter/struct.Parser.html
//! [tree-sitter]: https://tree-sitter.github.io/

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_mprx() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_mprx) };

/// Returns the tree-sitter [`tree_sitter::Language`] for this grammar.
///
/// This is a thin wrapper around [`LANGUAGE`] (a [`LanguageFn`]) for callers
/// that want the concrete `tree_sitter::Language` type directly, e.g. to pass
/// to [`tree_sitter::Parser::set_language`].
pub fn language() -> tree_sitter::Language {
    LANGUAGE.into()
}

/// The content of the [`node-types.json`] file for this grammar.
///
/// [`node-types.json`]: https://tree-sitter.github.io/tree-sitter/using-parsers/6-static-node-types
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

#[cfg(with_highlights_query)]
/// The syntax highlighting query for this grammar.
pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/highlights.scm");

#[cfg(with_injections_query)]
/// The language injection query for this grammar.
pub const INJECTIONS_QUERY: &str = include_str!("../../queries/injections.scm");

#[cfg(with_locals_query)]
/// The local variable query for this grammar.
pub const LOCALS_QUERY: &str = include_str!("../../queries/locals.scm");

#[cfg(with_tags_query)]
/// The symbol tagging query for this grammar.
pub const TAGS_QUERY: &str = include_str!("../../queries/tags.scm");

#[cfg(test)]
mod tests {
    #[test]
    fn test_can_load_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading Mprx parser");
    }

    #[test]
    fn test_language_wrapper() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::language())
            .expect("Error loading Mprx parser via language()");
        let tree = parser.parse(r#"<page title="Users" />"#, None).unwrap();
        assert!(!tree.root_node().has_error());
    }
}
