//! The input limits (decision G5): a source, an element count and an
//! attribute count past the limit are each one diagnostic at the first place
//! past it, no matter how the source is laid out, and the limit itself is
//! allowed.

use mesh_parser::{MAX_ATTRIBUTES, MAX_ELEMENTS, MAX_SOURCE_BYTES};

fn codes(source: &str) -> Vec<&'static str> {
    mesh_parser::parse(source)
        .errors
        .iter()
        .map(|error| error.code.as_str())
        .collect()
}

fn siblings(count: usize) -> String {
    format!("<page>{}</page>", "<t/>".repeat(count - 1))
}

fn attributes(count: usize) -> String {
    let list: Vec<String> = (0..count).map(|i| format!("p{i}=\"x\"")).collect();
    format!("<card {} />", list.join(" "))
}

#[test]
fn the_limits_are_g5s() {
    assert_eq!(MAX_SOURCE_BYTES, 64 * 1024);
    assert_eq!(MAX_ELEMENTS, 5_000);
    assert_eq!(MAX_ATTRIBUTES, 64);
}

#[test]
fn a_source_at_the_limit_is_read_and_one_byte_past_it_is_refused() {
    let at = format!("<a>{}</a>", "x".repeat(MAX_SOURCE_BYTES - 7));
    assert_eq!(at.len(), MAX_SOURCE_BYTES);
    assert!(codes(&at).is_empty());
    let past = format!("<a>{}</a>", "x".repeat(MAX_SOURCE_BYTES - 6));
    let parsed = mesh_parser::parse(&past);
    assert_eq!(codes(&past), ["input-too-large"]);
    assert_eq!(parsed.ast, None);
    // At the first byte past the limit, which is on a character boundary.
    assert_eq!(parsed.errors[0].span.start_byte, MAX_SOURCE_BYTES);
}

#[test]
fn the_limit_does_not_split_a_character() {
    let source = format!("<a>{}é</a>", "x".repeat(MAX_SOURCE_BYTES - 4));
    let parsed = mesh_parser::parse(&source);
    assert_eq!(codes(&source), ["input-too-large"]);
    assert!(source.is_char_boundary(parsed.errors[0].span.start_byte));
}

#[test]
fn elements_are_counted_to_the_limit() {
    let at = siblings(MAX_ELEMENTS);
    assert!(codes(&at).is_empty(), "{:?}", codes(&at));
    let past = siblings(MAX_ELEMENTS + 1);
    assert!(past.len() <= MAX_SOURCE_BYTES, "the count, not the size, is refused");
    let parsed = mesh_parser::parse(&past);
    assert_eq!(codes(&past), ["too-many-elements"]);
    assert_eq!(parsed.ast, None);
    // At the tag name of the first element past the limit: the last sibling.
    let span = parsed.errors[0].span;
    assert_eq!(&past[span.start_byte..span.end_byte], "t");
    assert_eq!(span.start_byte, past.rfind("<t/>").unwrap() + 1);
}

#[test]
fn attributes_are_counted_per_element_to_the_limit() {
    assert!(codes(&attributes(MAX_ATTRIBUTES)).is_empty());
    let past = attributes(MAX_ATTRIBUTES + 1);
    let parsed = mesh_parser::parse(&past);
    assert_eq!(codes(&past), ["too-many-attributes"]);
    let span = parsed.errors[0].span;
    assert_eq!(&past[span.start_byte..span.end_byte], "p64=\"x\"");
    // Each element has its own count.
    let two = format!("<a>{}{}</a>", attributes(MAX_ATTRIBUTES), attributes(MAX_ATTRIBUTES));
    assert!(codes(&two).is_empty());
}

#[test]
fn event_bindings_count_as_attributes() {
    let list: Vec<String> = (0..MAX_ATTRIBUTES).map(|i| format!("p{i}=\"x\"")).collect();
    let source = format!("<card {} on.tap={{save}} />", list.join(" "));
    assert_eq!(codes(&source), ["too-many-attributes"]);
}

#[test]
fn an_open_tag_with_content_counts_its_attributes_too() {
    let list: Vec<String> = (0..=MAX_ATTRIBUTES).map(|i| format!("p{i}=\"x\"")).collect();
    let source = format!("<card {}>x</card>", list.join(" "));
    assert_eq!(codes(&source), ["too-many-attributes"]);
}
