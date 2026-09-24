//! `context_at`: what kind of place the cursor is in (outline D4), one
//! test per rule and shape of the Pass 4 plan's table (K1–K6), written as
//! the text with `|` at the cursor. Each expectation is the context and
//! the text it would replace.

use mesh_parser::{context_at, Context};

/// The context at the `|` in `marked`, as `"Kind(detail) replacing 'text'"`.
fn context(marked: &str) -> Option<String> {
    let offset = marked.find('|').expect("a cursor");
    let source = marked.replacen('|', "", 1);
    let completion = context_at(&source, offset)?;
    let kind = match &completion.context {
        Context::TagName => "TagName".to_string(),
        Context::AttributeName { tag } => format!("AttributeName({tag})"),
        Context::EventName { tag } => format!("EventName({tag})"),
        Context::Handler => "Handler".to_string(),
        Context::Value => "Value".to_string(),
        Context::Member { object_span, .. } => format!(
            "Member({})",
            &source[object_span.start_byte..object_span.end_byte]
        ),
        other => format!("{other:?}"),
    };
    let replace = &source[completion.replace.start_byte..completion.replace.end_byte];
    Some(format!("{kind} replacing '{replace}'"))
}

fn in_page(inner: &str) -> String {
    format!("<page title=\"U\">\n  {inner}\n</page>")
}

fn assert_context(marked: &str, expected: Option<&str>) {
    assert_eq!(
        context(marked).as_deref(),
        expected,
        "at the | in {marked:?}"
    );
}

// K1: tag names.

#[test]
fn k1_after_a_less_than() {
    assert_context(&in_page("<avatar />\n  <|"), Some("TagName replacing ''")); // S15
    assert_context("<|", Some("TagName replacing ''")); // S18
    assert_context(
        &in_page("<av|\n  <text>hi</text>"),
        Some("TagName replacing 'av'"),
    ); // S7
    assert_context(&in_page("<user-c|"), Some("TagName replacing 'user-c'"));
    assert_context(&in_page("<avatar| />"), Some("TagName replacing 'avatar'"));
}

#[test]
fn k1_not_a_closing_tag_and_not_an_operator() {
    assert_context(&in_page("<text>hi</|"), None);
    assert_context(
        &in_page("<text>{compact <|}</text>"),
        Some("Value replacing ''"),
    );
}

// K2: event names.

#[test]
fn k2_after_on_dot() {
    assert_context(
        &in_page("<button on.| >x</button>"),
        Some("EventName(button) replacing ''"),
    ); // S11
    assert_context(
        &in_page("<button on.cl| >x</button>"),
        Some("EventName(button) replacing 'cl'"),
    ); // S12
    assert_context(
        &in_page("<button on.cl|ick={selectUser()}>x</button>"),
        Some("EventName(button) replacing 'click'"),
    );
    assert_context(
        &in_page("<button disabled={true} on.|"),
        Some("EventName(button) replacing ''"),
    );
}

#[test]
fn k2_needs_an_open_tag() {
    assert_context(&in_page("<text>on.|</text>"), None);
}

// K3: members.

#[test]
fn k3_after_a_dot() {
    assert_context(
        &in_page("<text>{user.|}</text>"),
        Some("Member(user) replacing ''"),
    ); // S1
    assert_context(
        &in_page("<text>{!user.|}</text>"),
        Some("Member(user) replacing ''"),
    ); // S3
    assert_context(
        &in_page("<text>{user.avatar.|}</text>"),
        Some("Member(user.avatar) replacing ''"),
    );
    assert_context(
        &in_page("<text>{compact ? user.| : \"x\"}</text>"),
        Some("Member(user) replacing ''"),
    ); // S4
    assert_context(
        &in_page("<avatar src={user.na|} alt=\"x\" />"),
        Some("Member(user) replacing 'na'"),
    ); // C-b
    assert_context(
        &in_page("<text>{selectUser(user.|)}</text>"),
        Some("Member(user) replacing ''"),
    );
}

#[test]
fn k3_before_an_unclosed_block_still_finds_the_object() {
    // S16: the whole file is an ERROR, and what follows is misparsed.
    assert_context(
        "<page title=\"U\">\n  <text>{user.|\n</page>",
        Some("Member(user) replacing ''"),
    );
    assert_context(
        "<page title=\"U\">\n  <text>{user.nam|\n</page>",
        Some("Member(user) replacing 'nam'"),
    );
}

// K4: attribute names.

#[test]
fn k4_in_an_opening_tag() {
    assert_context(
        &in_page("<avatar |"),
        Some("AttributeName(avatar) replacing ''"),
    ); // S5
    assert_context(
        &in_page("<avatar alt=\"x\" |"),
        Some("AttributeName(avatar) replacing ''"),
    ); // S6
    assert_context(
        &in_page("<avatar s|"),
        Some("AttributeName(avatar) replacing 's'"),
    ); // S8
    assert_context(
        &in_page("<avatar alt=\"x\" s| />"),
        Some("AttributeName(avatar) replacing 's'"),
    ); // S10
    assert_context(
        &in_page("<avatar alt=\"x\" | />"),
        Some("AttributeName(avatar) replacing ''"),
    ); // C-a
    assert_context(
        &in_page("<avatar src={user.avatar} | />"),
        Some("AttributeName(avatar) replacing ''"),
    );
    assert_context(
        &in_page("<avatar al|t=\"x\" />"),
        Some("AttributeName(avatar) replacing 'alt'"),
    );
}

#[test]
fn k4_not_in_a_value_text_or_after_the_tag() {
    assert_context(&in_page("<avatar src=|"), None); // S9
    assert_context(&in_page("<avatar alt=\"x|\" />"), None);
    assert_context(&in_page("<text>hel|lo</text>"), None);
    assert_context(&in_page("<text> |</text>"), None);
    assert_context(&in_page("<avatar /> |"), None);
}

// K5 and K6: expressions.

#[test]
fn k5_at_the_start_of_a_handler() {
    assert_context(
        &in_page("<button on.click={|} >x</button>"),
        Some("Handler replacing ''"),
    ); // S13
    assert_context(
        &in_page("<button on.click={ | }>x</button>"),
        Some("Handler replacing ''"),
    );
    assert_context(
        &in_page("<button on.click={sel|}>x</button>"),
        Some("Handler replacing 'sel'"),
    ); // C-c
}

#[test]
fn k6_where_a_value_starts() {
    assert_context(&in_page("<text>{|}</text>"), Some("Value replacing ''"));
    assert_context(&in_page("<text>{ | }</text>"), Some("Value replacing ''")); // S13
    assert_context(&in_page("<text>{us|}</text>"), Some("Value replacing 'us'"));
    assert_context(
        &in_page("<text>{user.name + |}</text>"),
        Some("Value replacing ''"),
    ); // S13
    assert_context(
        &in_page("<button on.click={selectUser(|)}>x</button>"),
        Some("Value replacing ''"),
    ); // C-d
    assert_context(
        &in_page("<text>{[1, |]}</text>"),
        Some("Value replacing ''"),
    );
    assert_context(
        &in_page("<text>{compact ? us| : \"x\"}</text>"),
        Some("Value replacing 'us'"),
    );
    // With nothing after it, the `?` is an ERROR token: no context (K0).
    assert_context(&in_page("<text>{compact ? |}</text>"), None);
    assert_context(&in_page("<avatar size={!|} />"), Some("Value replacing ''"));
}

#[test]
fn k0_object_keys_and_errors() {
    assert_context(&in_page("<text>{{ na| }}</text>"), None); // C-e
    assert_context(&in_page("<text>{{ |}}</text>"), None);
    assert_context("|", None);
    assert_context(
        "<page title=\"U\">\n  <avatar alt=\"x />\n  <text>{|user.name}</text>\n</page>",
        None,
    ); // S17
}

#[test]
fn every_offset_of_every_fixture_classifies_without_panicking() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = vec![
        examples.join("users-page.mprx"),
        examples.join("user-card.mprx"),
    ];
    for dir in [
        "fixtures/pass",
        "fixtures/fail",
        "fixtures/check/pass",
        "fixtures/check/fail",
    ] {
        for entry in std::fs::read_dir(examples.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "mprx") {
                files.push(path);
            }
        }
    }
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        for offset in 0..=source.len() + 2 {
            context_at(&source, offset);
            // And every prefix, as it's typed.
            if source.is_char_boundary(offset.min(source.len())) {
                let prefix = &source[..offset.min(source.len())];
                context_at(prefix, prefix.len());
            }
        }
    }
}
