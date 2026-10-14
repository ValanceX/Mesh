use mesh_format::{format, format_range_edits, indent_for_line, Error, Options};

fn two() -> Options {
    Options::default()
}

fn four() -> Options {
    Options {
        tab_size: 4,
        use_tabs: false,
    }
}

fn fmt(source: &str) -> String {
    format(source, &two()).expect("formats")
}

#[test]
fn indents_by_nesting_and_ends_with_one_newline() {
    let source = "<page>\n<section>\n      <note>hi</note>\n   </section>\n</page>   \n\n\n";
    assert_eq!(
        fmt(source),
        "<page>\n  <section>\n    <note>hi</note>\n  </section>\n</page>\n"
    );
}

#[test]
fn a_tag_over_several_lines_gets_one_attribute_per_line() {
    let source = "<button\n    class=\"theme-toggle\" pressed={theme == \"dark\" ? \"true\" : \"false\"}\n    on.click={toggle()}\n>\n    {theme == \"dark\" ? \"Light\" : \"Dark\"}\n</button>\n";
    let expected = "<button\n    class=\"theme-toggle\"\n    pressed={theme == \"dark\" ? \"true\" : \"false\"}\n    on.click={toggle()}\n>\n    {theme == \"dark\" ? \"Light\" : \"Dark\"}\n</button>\n";
    assert_eq!(format(source, &four()).unwrap(), expected);
}

#[test]
fn a_tag_on_one_line_stays_on_one_line() {
    let source = "<page>\n<a href=\"x\" class=\"y\" />\n</page>\n";
    assert_eq!(
        fmt(source),
        "<page>\n  <a href=\"x\" class=\"y\" />\n</page>\n"
    );
}

#[test]
fn a_self_closing_tag_over_several_lines_closes_on_its_own_line() {
    let source = "<page>\n<a\nhref=\"x\"\nclass=\"y\" />\n</page>\n";
    assert_eq!(
        fmt(source),
        "<page>\n  <a\n    href=\"x\"\n    class=\"y\"\n  />\n</page>\n"
    );
}

#[test]
fn blocks_indent_their_branches_and_outdent_their_markers() {
    let source = "<list>\n{#if ready}\n<a />\n{:else if !failed && n > 1}\n<b />\n{:else}\n<c />\n{/if}\n{#each users as user (user.id)}\n<i>{user.name}</i>\n{/each}\n</list>\n";
    let expected = "<list>\n  {#if ready}\n    <a />\n  {:else if !failed && n > 1}\n    <b />\n  {:else}\n    <c />\n  {/if}\n  {#each users as user (user.id)}\n    <i>{user.name}</i>\n  {/each}\n</list>\n";
    assert_eq!(fmt(source), expected);
}

#[test]
fn text_with_content_is_never_touched() {
    let source = "<p>\n   hello   world\n      and more\n</p>\n";
    // The text node's whitespace is meaning (the spec keeps it verbatim).
    assert_eq!(fmt(source), source);
}

#[test]
fn whitespace_between_tokens_without_a_newline_is_kept() {
    let source = "<p><b>x</b> <i>y</i></p>\n";
    assert_eq!(fmt(source), source);
}

#[test]
fn a_blank_line_between_children_is_kept_once() {
    let source = "<page>\n<a />\n\n\n\n<b />\n</page>\n";
    assert_eq!(fmt(source), "<page>\n  <a />\n\n  <b />\n</page>\n");
}

#[test]
fn tabs_when_asked() {
    let options = Options {
        tab_size: 4,
        use_tabs: true,
    };
    assert_eq!(
        format("<a>\n<b />\n</a>\n", &options).unwrap(),
        "<a>\n\t<b />\n</a>\n"
    );
}

#[test]
fn it_is_idempotent() {
    for source in [
        "<page>\n<section>\n      <note>hi</note>\n   </section>\n</page>   \n",
        "<a\nx=\"1\"\n  y={2}\n>\n{#if a}\n<b />\n{/if}\n</a>",
        "<list>\n{#each xs as x (x.id)}\n<i\nclass=\"c\"\n>{x}</i>\n{/each}\n</list>\n",
    ] {
        let once = fmt(source);
        assert_eq!(fmt(&once), once, "{source:?}");
    }
}

#[test]
fn a_source_with_errors_is_not_formatted() {
    assert_eq!(format("<page>\n<a\n</page>\n", &two()), Err(Error::Syntax));
    assert_eq!(format("<a>{#if x}<b /></a>\n", &two()), Err(Error::Syntax));
}

#[test]
fn a_selection_formats_only_what_it_touches() {
    let source = "<page>\n<a />\n<b />\n   <c />\n</page>\n";
    // From the start of `<b />` to the end of `<c />`: the gaps in it, not the one before `<a />`.
    let start = source.find("<b").unwrap();
    let end = source.find("<c").unwrap() + 4;
    let edits = format_range_edits(source, start, end, &two()).unwrap();
    let mut out = source.to_string();
    for edit in edits.iter().rev() {
        out.replace_range(edit.start..edit.end, &edit.text);
    }
    assert_eq!(out, "<page>\n<a />\n  <b />\n  <c />\n</page>\n");
}

#[test]
fn every_valid_example_and_fixture_formats_and_stays_formatted() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seen = 0;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "mprx") {
                let source = std::fs::read_to_string(&path).unwrap();
                match format(&source, &two()) {
                    Ok(once) => {
                        seen += 1;
                        assert_eq!(format(&once, &two()).unwrap(), once, "{}", path.display());
                    }
                    Err(Error::Syntax) => {} // a fixture that is meant to fail
                    Err(other) => panic!("{}: {other:?}", path.display()),
                }
            }
        }
    }
    assert!(seen >= 10, "only {seen} files found");
}

#[test]
fn enter_between_a_tag_and_its_closer_indents_the_new_line() {
    let source = "<page>\n<a>\n\n</a>\n</page>\n";
    let line = source.find("\n\n").unwrap() + 1; // the empty line
    assert_eq!(indent_for_line(source, line, &two()), "    ");
}

#[test]
fn a_line_that_closes_is_one_level_out() {
    let source = "<page>\n<a>\n  </a>\n</page>\n";
    let line = source.find("  </a>").unwrap();
    assert_eq!(indent_for_line(source, line, &two()), "  ");
}

#[test]
fn a_new_line_inside_an_open_tag_is_an_attribute_line_and_a_closer_is_the_tags() {
    let source = "<page>\n<a\n\n>\n</a>\n</page>\n";
    let line = source.find("\n\n").unwrap() + 1;
    assert_eq!(indent_for_line(source, line, &two()), "    ");
    let closer = "<page>\n<a\n  x=\"1\"\n  >\n</a>\n</page>\n";
    let at = closer.find("  >").unwrap();
    assert_eq!(indent_for_line(closer, at, &two()), "  ");
}

#[test]
fn the_new_line_after_a_block_opener_and_before_a_marker() {
    let source = "<page>\n{#if a}\n\n{:else}\n<b />\n{/if}\n</page>\n";
    let line = source.find("\n\n").unwrap() + 1;
    assert_eq!(indent_for_line(source, line, &two()), "    ");
    let marker = source.find("{:else}").unwrap();
    assert_eq!(indent_for_line(source, marker, &two()), "  ");
}

#[test]
fn a_new_line_in_the_middle_of_text_keeps_the_previous_indentation() {
    let source = "<p>\n    hello\n\n</p>\n";
    let line = source.find("\n\n").unwrap() + 1;
    assert_eq!(indent_for_line(source, line, &two()), "    ");
}
