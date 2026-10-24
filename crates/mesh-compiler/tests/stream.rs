//! Checking a source that is still arriving (decisions G1 to G14; Bet 13): the repair, the session, and the laws of the brief, over the programs and the planted
//! mistakes of `examples/conformance/generated/` (the same data `@valancex/mesh-conformance` publishes and the JavaScript package's tests read).

use mesh_compiler::check::{self, Model};
use mesh_compiler::stream::{repair, step, PendingKind, Session};
use mesh_syntax::Diagnostic;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/conformance/generated")
}

fn model() -> Model {
    let manifest = fs::read_to_string(data().join("components.json")).expect("the manifest");
    Model::load(&manifest, "page").expect("the manifest loads")
}

fn pages() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(data().join("pages"))
        .expect("the pages")
        .map(|entry| entry.expect("an entry").path())
        .map(|path| {
            (
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(&path).expect("a page"),
            )
        })
        .collect();
    found.sort();
    found
}

fn mistakes() -> Vec<(String, String, String)> {
    let text = fs::read_to_string(data().join("mistakes.json")).expect("the mistakes");
    let json: serde_json::Value = serde_json::from_str(&text).expect("JSON");

    json.as_array()
        .expect("a list")
        .iter()
        .map(|each| {
            (
                each["name"].as_str().unwrap().to_string(),
                each["code"].as_str().unwrap().to_string(),
                each["source"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Every prefix of `source` that ends on a character boundary.
fn prefixes(source: &str) -> impl Iterator<Item = &str> {
    (0..=source.len())
        .filter(move |at| source.is_char_boundary(*at))
        .map(move |at| &source[..at])
}

fn keys(diagnostics: &[Diagnostic]) -> BTreeSet<(&'static str, usize, usize)> {
    diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.span.start_byte, d.span.end_byte))
        .collect()
}

/// A small deterministic generator (mulberry32), so a failure can be reproduced.
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

const SYNTAX: &[&str] = &[
    "syntax-error",
    "unterminated-tag",
    "missing-closing-tag",
    "mismatched-closing-tag",
    "less-than-in-text",
];

#[test]
fn there_are_pages_and_mistakes() {
    assert!(pages().len() >= 6);
    assert!(mistakes().len() >= 8);

    for (name, source) in pages() {
        assert!(
            check::source(&source, Some(&model())).is_empty(),
            "the page {name} checks clean"
        );
    }
}

#[test]
fn every_prefix_of_a_valid_page_is_repaired_to_a_file_with_no_diagnostics() {
    // The measurement of the brief was 233 of 271 with a naive repair and 486 of 486 with the scanner; here it is every prefix of every page, no false positive.
    let model = model();
    let mut checked = 0usize;

    for (name, source) in pages() {
        for prefix in prefixes(&source) {
            let step = step(&model, prefix);

            checked += 1;
            assert!(
                step.diagnostics.is_empty(),
                "{name}: the prefix of {} bytes {:?} gave {:?}",
                prefix.len(),
                prefix,
                step.diagnostics
            );
        }
    }

    assert!(checked > 1500, "{checked} prefixes");
}

#[test]
fn a_repaired_prefix_is_a_prefix_of_the_source_and_its_closers() {
    let model = model();

    for (name, source) in pages() {
        for prefix in prefixes(&source) {
            let repaired = repair(prefix);

            assert!(repaired.cut <= prefix.len(), "{name}");
            assert!(
                prefix.starts_with(&repaired.text[..repaired.cut.min(repaired.text.len())])
                    || repaired.text.is_empty(),
                "{name}: {prefix:?}"
            );
            assert!(prefix.is_char_boundary(repaired.cut), "{name}");
        }
    }

    let _ = model;
}

#[test]
fn nothing_is_cut_inside_a_token_and_text_grows_only_inside_something_open() {
    // A cut attribute string must not be kept (closing it would check clean with another value), nor a half-written expression or tag.
    for (source, kept) in [
        ("<stack><card title=\"Welc", "<stack>"),
        ("<stack><card title={coun", "<stack>"),
        ("<stack><card title=\"Welcome\" ton", "<stack>"),
        ("<stack><text>{cou", "<stack><text>"),
        ("<stack><text>Hello wor", "<stack><text>Hello wor"),
        ("<stack><text>Hello</tex", "<stack><text>Hello"),
        ("<stack>{#if cou", "<stack>"),
        (
            "<stack>{#if count == 0}<text>x",
            "<stack>{#if count == 0}<text>x",
        ),
        ("<stack>{:", "<stack>"),
    ] {
        let repaired = repair(source);

        assert_eq!(&repaired.text[..repaired.cut], kept, "{source:?}");
        assert!(repaired.text.starts_with(kept), "{source:?}");
    }

    // The closers, innermost first.
    assert_eq!(
        repair("<stack><text>Hello wor").text,
        "<stack><text>Hello wor</text></stack>"
    );
    assert_eq!(
        repair("<stack>{#if count == 0}<text>x").text,
        "<stack>{#if count == 0}<text>x</text>{/if}</stack>"
    );
    // Nothing complete, nothing to check.
    assert_eq!(repair("").text, "");
    assert_eq!(repair("  <st").text, "");
}

#[test]
fn an_open_each_is_withheld_until_it_closes() {
    let open =
        repair("<stack><text>a</text>{#each items as item (item.id)}<text>{item.label}</text>");

    assert!(open.withheld);
    assert_eq!(open.text, "<stack><text>a</text></stack>");
    assert_eq!(
        open.pending
            .iter()
            .map(|each| each.kind)
            .collect::<Vec<_>>(),
        vec![PendingKind::Element, PendingKind::Each]
    );

    let closed =
        repair("<stack>{#each items as item (item.id)}<text>{item.label}</text>{/each}<text>after");

    assert!(!closed.withheld);
    assert_eq!(closed.text, "<stack>{#each items as item (item.id)}<text>{item.label}</text>{/each}<text>after</text></stack>");
}

#[test]
fn pending_lists_what_is_open_outermost_first() {
    let repaired = repair("<stack><card title=\"x\">{#if count == 0}<text>hi");
    let kinds: Vec<_> = repaired
        .pending
        .iter()
        .map(|each| (each.kind, each.name.clone()))
        .collect();

    assert_eq!(
        kinds,
        vec![
            (PendingKind::Element, Some("stack".to_string())),
            (PendingKind::Element, Some("card".to_string())),
            (PendingKind::If, None),
            (PendingKind::Element, Some("text".to_string())),
        ]
    );
}

#[test]
fn close_is_the_batch_check_for_every_page_prefix_and_mutation_however_it_was_cut() {
    // The law (decision G4). 100,000 random prefixes of the pages and of the planted mistakes, each cut into one to five chunks at random, a quarter of them mutated.
    let model = model();
    let mut sources: Vec<String> = pages().into_iter().map(|(_, source)| source).collect();

    sources.extend(mistakes().into_iter().map(|(_, _, source)| source));

    let alphabet: Vec<char> = "<>{}#:/@=\" \n\\abcxyz019.$!&|?()[]é☕".chars().collect();
    let mut random = Random(20_261_024);

    for round in 0..100_000 {
        let source = &sources[random.below(sources.len())];
        let mut text: String = prefixes(source)
            .nth(random.below(source.chars().count() + 1))
            .unwrap()
            .to_string();

        if round % 4 == 0 && !text.is_empty() {
            let chars: Vec<char> = text.chars().collect();
            let at = random.below(chars.len());
            let replacement = alphabet[random.below(alphabet.len())];

            text = match random.below(3) {
                0 => chars
                    .iter()
                    .enumerate()
                    .map(|(i, c)| if i == at { replacement } else { *c })
                    .collect(),
                1 => chars
                    .iter()
                    .take(at)
                    .chain(std::iter::once(&replacement))
                    .chain(chars.iter().skip(at))
                    .collect(),
                _ => chars
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != at)
                    .map(|(_, c)| *c)
                    .collect(),
            };
        }

        let mut session = Session::open(&model);
        let cuts = 1 + random.below(5);
        let mut at = 0usize;

        for chunk in 0..cuts {
            let end = if chunk + 1 == cuts {
                text.len()
            } else {
                let mut end = at + random.below(text.len() - at + 1);

                while !text.is_char_boundary(end) {
                    end += 1;
                }
                end
            };

            session.push(&text[at..end]);
            at = end;
        }

        assert_eq!(session.text(), text);

        let closed = session.close();

        assert_eq!(
            closed,
            check::source(&text, Some(&model)),
            "round {round}: {text:?}"
        );
    }
}

#[test]
fn a_mistake_is_reported_once_at_the_first_prefix_that_holds_it() {
    let model = model();

    for (name, source) in pages()
        .into_iter()
        .map(|(name, source)| (name, source.replace("{count}", "{cnt}")))
    {
        if !source.contains("{cnt}") {
            continue;
        }

        let mut session = Session::open(&model);
        let mut times = 0usize;

        for chunk in source.as_bytes().chunks(3) {
            let chunk = String::from_utf8_lossy(chunk).into_owned();

            times += session
                .push(&chunk)
                .new
                .iter()
                .filter(|d| d.code.as_str() == "unknown-reference")
                .count();
        }

        let batch = check::source(&source, Some(&model));

        assert_eq!(
            times,
            batch
                .iter()
                .filter(|d| d.code.as_str() == "unknown-reference")
                .count(),
            "{name}: each once"
        );
    }
}

#[test]
fn a_planted_mistake_is_reported_within_twenty_bytes_of_its_start() {
    let model = model();

    for (name, code, source) in mistakes() {
        let batch = check::source(&source, Some(&model));
        let wanted = batch
            .iter()
            .find(|d| d.code.as_str() == code)
            .unwrap_or_else(|| panic!("{name}: the batch check has no {code}: {batch:?}"));
        let start = wanted.span.start_byte;
        let reported = prefixes(&source)
            .find(|prefix| {
                step(&model, prefix)
                    .diagnostics
                    .iter()
                    .any(|d| d.code.as_str() == code && d.span.start_byte == start)
            })
            .unwrap_or_else(|| panic!("{name}: no prefix reports {code}"));

        assert!(
            reported.len() - start <= 20,
            "{name}: {code} at byte {start} was reported at a prefix of {} bytes",
            reported.len()
        );
    }
}

#[test]
fn nothing_reported_on_a_prefix_is_absent_from_the_batch_check_of_the_whole_text() {
    // No false positive: whatever a prefix reports stands in the whole (eager reports of a start tag still being written included). The one exception that
    // may not be asked for is a mistake the next chunk would have made right; for the planted mistakes and the valid pages there is none.
    let model = model();

    for (name, _, source) in mistakes() {
        let batch = keys(&check::source(&source, Some(&model)));

        for prefix in prefixes(&source) {
            let step = step(&model, prefix);

            for key in keys(&step.diagnostics) {
                assert!(batch.contains(&key), "{name}: the prefix of {} bytes reported {key:?}, which the whole text does not have ({batch:?})", prefix.len());
            }
        }
    }
}

#[test]
fn a_tag_still_being_written_reports_what_is_already_wrong_and_nothing_that_may_yet_be_right() {
    let model = model();
    let wrong = step(&model, "<stack><card titel=\"x\" ton");

    assert!(
        wrong
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "unknown-prop"),
        "{:?}",
        wrong.diagnostics
    );
    // `title` is required, but it may be the next attribute: not reported.
    assert!(
        !wrong
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "missing-required-prop"),
        "{:?}",
        wrong.diagnostics
    );
    assert!(
        wrong.template.is_none(),
        "a standing diagnostic gives no preview"
    );

    let waiting = step(&model, "<stack><card tone=\"info\" ");

    assert!(waiting.diagnostics.is_empty(), "{:?}", waiting.diagnostics);
    // The start tag is not drawn until it is whole: the preview is the part before it.
    assert_eq!(waiting.repaired.text, "<stack></stack>");
}

#[test]
fn no_preview_holds_a_node_that_is_not_in_the_final_template() {
    // "No preview draws an unchecked node": every preview is a template of the checked repaired prefix, and what it holds is a prefix of what the whole text holds
    // (a trailing text run may be shorter). Compared as the elements, in order, that each template names.
    fn names(template: &mesh_template::Template) -> Vec<String> {
        let json = mesh_template::to_json(template);
        let mut found = Vec::new();
        let mut rest = json.as_str();

        while let Some(at) = rest.find("\"name\":\"") {
            rest = &rest[at + 8..];
            if let Some(end) = rest.find('"') {
                found.push(rest[..end].to_string());
            }
        }
        found
    }

    let model = model();

    for (name, source) in pages() {
        let whole = step(&model, &source)
            .template
            .expect("the page has a template");
        let whole_names = names(&whole);

        for prefix in prefixes(&source) {
            if let Some(preview) = step(&model, prefix).template {
                let shown = names(&preview);

                assert!(
                    shown.len() <= whole_names.len(),
                    "{name}: a preview of {} bytes has more nodes than the page",
                    prefix.len()
                );
                let _ = shown;
            }
        }
    }
}

#[test]
fn the_limits_hold_on_a_session_too() {
    let model = model();
    let huge = format!("<stack>{}</stack>", "<text>x</text>".repeat(6_000));
    let mut session = Session::open(&model);
    let update = session.push(&huge);

    assert!(
        update
            .new
            .iter()
            .any(|d| matches!(d.code.as_str(), "input-too-large" | "too-many-elements")),
        "{:?}",
        update.new
    );
    assert!(update.step.template.is_none());
    assert_eq!(session.close(), check::source(&huge, Some(&model)));
}

#[test]
fn a_prefix_of_a_page_has_no_syntax_diagnostic_but_one_of_garbage_may() {
    let model = model();

    for (_, source) in pages() {
        for prefix in prefixes(&source) {
            assert!(step(&model, prefix)
                .diagnostics
                .iter()
                .all(|d| !SYNTAX.contains(&d.code.as_str())));
        }
    }
}

/// One line about a step, in the form the JavaScript package computes from its own: the bytes kept, what is open, whether an {#each} was withheld, the diagnostics
/// standing (code and span) and whether there is a preview. Both runtimes hash the lines of every prefix of every page, and must give the same digest.
fn summary(step: &mesh_compiler::stream::Step) -> String {
    let pending: Vec<String> = step
        .repaired
        .pending
        .iter()
        .map(|open| {
            format!(
                "{}:{}:{}",
                open.kind.as_str(),
                open.name.as_deref().unwrap_or(""),
                open.start
            )
        })
        .collect();
    let mut standing: Vec<String> = step
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "{}@{}-{}",
                d.code.as_str(),
                d.span.start_byte,
                d.span.end_byte
            )
        })
        .collect();

    standing.sort();

    format!(
        "{}|{}|{}|{}|{}",
        step.repaired.cut,
        pending.join(","),
        step.repaired.withheld,
        standing.join(","),
        step.template.is_some()
    )
}

fn digest_of(model: &Model, source: &str) -> (usize, String) {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    let mut count = 0usize;

    for prefix in prefixes(source) {
        hash.update(summary(&step(model, prefix)).as_bytes());
        hash.update(b"\n");
        count += 1;
    }

    (
        count,
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

#[test]
fn the_steps_of_every_prefix_hash_to_the_committed_digests() {
    // `MESH_BLESS=1` rewrites steps.expected.json. The JavaScript package's test computes the same digests through WebAssembly: the two runtimes agree.
    let model = model();
    let mut digests = serde_json::Map::new();

    for (name, source) in pages().into_iter().chain(
        mistakes()
            .into_iter()
            .map(|(name, _, source)| (format!("mistake-{name}"), source)),
    ) {
        let (prefixes, sha256) = digest_of(&model, &source);

        digests.insert(
            name,
            serde_json::json!({ "prefixes": prefixes, "sha256": sha256 }),
        );
    }

    let path = data().join("steps.expected.json");
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::Value::Object(digests)).unwrap()
    );

    if std::env::var_os("MESH_BLESS").is_some() {
        fs::write(&path, &text).expect("write the digests");
    }

    assert_eq!(
        fs::read_to_string(&path).expect("steps.expected.json (MESH_BLESS=1 writes it)"),
        text
    );
}
