//! `mesh_compiler::editor`: recovery for editors, and the properties the
//! recovery spec requires of it (P1 and P2).

use mesh_analysis::Target;
use mesh_compiler::{compile, compile_with, editor, CompileOptions};
use std::fs;
use std::path::{Path, PathBuf};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn mprx_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(examples().join(dir))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "mprx"))
        .collect();
    files.sort();
    files
}

/// Every template in the corpus with its manifest and component.
fn corpus() -> Vec<(PathBuf, String, String)> {
    let example_manifest = read(&examples().join("components.json"));
    let check_manifest = read(&examples().join("fixtures/check/components.json"));
    let mut runs = vec![
        (
            examples().join("users-page.mprx"),
            example_manifest.clone(),
            "users-page".to_string(),
        ),
        (
            examples().join("user-card.mprx"),
            example_manifest.clone(),
            "user-card-example".to_string(),
        ),
        (
            examples().join("page.mprx"),
            example_manifest,
            "page".to_string(),
        ),
    ];
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        for file in mprx_files(dir) {
            runs.push((file, check_manifest.clone(), "template".to_string()));
        }
    }
    runs
}

/// P1: on a file that parses, recovery is one part, and it answers
/// exactly what the canonical analysis answers, at every offset.
#[test]
fn recovery_equals_canonical_analysis_on_every_parsing_file() {
    let mut compared = 0;
    for (file, manifest, component) in corpus() {
        let source = read(&file);
        let manifest = mesh_manifest::load(&manifest).unwrap();
        let template = manifest.template(&component).unwrap();
        let result = compile_with(&source, &CompileOptions::with_template(template));
        let Some(analysis) = &result.analysis else {
            continue;
        };
        let recovery = editor::recover(&source, template);
        assert_eq!(recovery.len(), 1, "{}", file.display());
        for offset in 0..=source.len() {
            assert_eq!(
                recovery.resolution_at(offset),
                analysis.resolution_at(offset),
                "{} at {offset}",
                file.display()
            );
            assert_eq!(
                recovery.typed_at(offset),
                analysis.typed_at(offset),
                "{} at {offset}",
                file.display()
            );
        }
        compared += 1;
    }
    assert!(compared > 20, "only {compared} files compared");
}

/// P2: recovery never changes what the compiler says, for any broken file
/// or any prefix of the examples, with or without a model.
#[test]
fn compile_with_is_unchanged_by_recovery() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let mut sources: Vec<String> = mprx_files("fixtures/fail")
        .iter()
        .map(|file| read(file))
        .collect();
    for example in ["users-page.mprx", "user-card.mprx"] {
        let source = read(&examples().join(example));
        sources.extend(
            source
                .char_indices()
                .map(|(end, _)| source[..end].to_string()),
        );
    }
    for source in &sources {
        let with = CompileOptions::with_template(template);
        let before = (compile(source), compile_with(source, &with));
        let recovery = editor::recover(source, template);
        let after = (compile(source), compile_with(source, &with));
        assert_eq!(before, after, "{source:?}");
        if before.1.ir.is_some() {
            assert_eq!(recovery.len(), 1, "{source:?}");
        }
    }
}

#[test]
fn the_innermost_part_wins() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let source = "<page title=\"U\">\n  <avatar src={user.} alt={user.name} />\n</page>";
    let recovery = editor::recover(source, template);
    // The page (an element part) and `user` (an expression part pruned
    // from inside it).
    assert_eq!(recovery.len(), 2);
    let user = source.find("user").unwrap() + 1;
    assert_eq!(
        recovery.resolution_at(user).map(|r| &r.target),
        Some(&Target::Scope("user".to_string()))
    );
    let alt = source.find("alt").unwrap() + 1;
    assert_eq!(
        recovery.resolution_at(alt).map(|r| &r.target),
        Some(&Target::Prop {
            component: "avatar".to_string(),
            prop: "alt".to_string()
        })
    );
    let name = source.find("name").unwrap() + 1;
    assert_eq!(
        recovery.typed_at(name).map(|t| t.ty.to_string()),
        Some("string".to_string())
    );
}

#[test]
fn a_file_nested_too_deep_recovers_nothing() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let depth = mesh_parser::MAX_NESTING_DEPTH + 72;
    let source = format!(
        "{}{{user.}}{}",
        "<text>".repeat(depth),
        "</text>".repeat(depth)
    );
    assert!(editor::recover(&source, template).is_empty());
}

// --- Completion candidates (outline D4) ---------------------------------

use mesh_compiler::editor::{candidates, Candidate, CandidateKind};
use std::collections::BTreeSet;

/// The codes that say a name isn't accepted where it's written.
const REJECTED: [&str; 8] = [
    "unknown-component",
    "unknown-prop",
    "unknown-event",
    "unknown-reference",
    "unknown-command",
    "unknown-member",
    "command-outside-handler",
    "handler-not-command",
];

/// The candidates at the `|` in `marked`, as `component`'s template,
/// with the canonical analysis if the text has one.
fn offered(marked: &str, component: &str) -> Vec<Candidate> {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template(component).unwrap();
    let offset = marked.find('|').unwrap();
    let source = marked.replacen('|', "", 1);
    let result = compile_with(&source, &CompileOptions::with_template(template));
    candidates(&source, offset, template, result.analysis.as_ref())
}

/// Soundness: each candidate, written in place of `⟨⟩` in `complete`, is
/// accepted: no diagnostic that rejects a name overlaps it.
fn assert_accepted(
    candidates: &[Candidate],
    component: &str,
    complete: impl Fn(&Candidate) -> String,
) {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template(component).unwrap();
    assert!(!candidates.is_empty());
    for candidate in candidates {
        let with_hole = complete(candidate);
        let start = with_hole.find('⟨').unwrap();
        let source = with_hole.replacen("⟨⟩", &candidate.label, 1);
        let end = start + candidate.label.len();
        let result = compile_with(&source, &CompileOptions::with_template(template));
        assert!(result.ir.is_some(), "{source}");
        for diagnostic in &result.diagnostics {
            let overlaps = diagnostic.span.start_byte < end && start < diagnostic.span.end_byte;
            assert!(
                !(overlaps && REJECTED.contains(&diagnostic.code.as_str())),
                "{} is offered but rejected in {source:?}: {}",
                candidate.label,
                diagnostic.message
            );
        }
    }
}

fn labels(candidates: &[Candidate]) -> Vec<&str> {
    candidates.iter().map(|c| c.label.as_str()).collect()
}

fn page(inner: &str) -> String {
    format!("<page title=\"U\">\n  {inner}\n</page>")
}

#[test]
fn every_candidate_is_accepted_by_the_compiler() {
    let tags = offered(&page("<|"), "users-page");
    assert_accepted(&tags, "users-page", |_| page("<⟨⟩ />"));

    for tag in ["avatar", "button", "user-card"] {
        let attributes = offered(&page(&format!("<{tag} |")), "users-page");
        assert_accepted(&attributes, "users-page", |candidate| {
            match candidate.kind {
                CandidateKind::Event => page(&format!("<{tag} ⟨⟩={{selectUser($event)}} />")),
                _ => page(&format!("<{tag} ⟨⟩={{user.name}} />")),
            }
        });
    }

    let events = offered(&page("<button on.|"), "users-page");
    assert_accepted(&events, "users-page", |_| {
        page("<button on.⟨⟩={selectUser($event)}>x</button>")
    });

    let commands = offered(&page("<button on.click={|}>x</button>"), "users-page");
    assert_accepted(&commands, "users-page", |_| {
        page("<button on.click={⟨⟩(user)}>x</button>")
    });

    let scope = offered(&page("<text>{|}</text>"), "users-page");
    assert_accepted(&scope, "users-page", |_| page("<text>{⟨⟩}</text>"));

    for marked in [
        page("<text>{user.|}</text>"),
        page("<text>{user.na|}</text>"),
    ] {
        let members = offered(&marked, "users-page");
        assert_accepted(&members, "users-page", |_| page("<text>{user.⟨⟩}</text>"));
    }
    let members = offered(
        "<user-card user={user} compact={layout.|} />",
        "user-card-example",
    );
    assert_accepted(&members, "user-card-example", |_| {
        "<user-card user={user} compact={layout.⟨⟩} />".to_string()
    });
}

/// Completeness: each context offers exactly the declarations D4 names.
#[test]
fn every_declaration_is_offered_where_it_may_be_written() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let set = |candidates: Vec<Candidate>| -> BTreeSet<String> {
        candidates.into_iter().map(|c| c.label).collect()
    };
    let keys = |names: Vec<&String>| -> BTreeSet<String> { names.into_iter().cloned().collect() };

    assert_eq!(
        set(offered(&page("<|"), "users-page")),
        keys(manifest.components().keys().collect())
    );
    let button = &manifest.components()["button"];
    let mut expected = keys(button.props.keys().collect());
    expected.extend(button.events.keys().map(|event| format!("on.{event}")));
    assert_eq!(set(offered(&page("<button |"), "users-page")), expected);
    assert_eq!(
        set(offered(&page("<button on.|"), "users-page")),
        keys(button.events.keys().collect())
    );
    let users_page = &manifest.components()["users-page"];
    assert_eq!(
        set(offered(
            &page("<button on.click={|}>x</button>"),
            "users-page"
        )),
        keys(users_page.commands.keys().collect())
    );
    assert_eq!(
        set(offered(&page("<text>{|}</text>"), "users-page")),
        keys(users_page.scope.keys().collect())
    );
    assert_eq!(
        labels(&offered(&page("<text>{user.|}</text>"), "users-page")),
        ["active", "avatar", "name"]
    );
}

#[test]
fn props_come_required_first_then_by_name() {
    let avatar = offered(&page("<avatar |"), "users-page");
    assert_eq!(labels(&avatar), ["alt", "src", "size"]);
    let required: Vec<bool> = avatar.iter().map(|c| c.required).collect();
    assert_eq!(required, [true, true, false]);
    assert_eq!(avatar[1].detail.as_deref(), Some("string?"));
}

#[test]
fn a_member_uses_the_canonical_type_when_there_is_one() {
    // `user.na` parses, so the canonical analysis types `user`.
    let members = offered(&page("<text>{user.na|}</text>"), "users-page");
    assert_eq!(labels(&members), ["active", "avatar", "name"]);
    assert_eq!(members[0].replace, {
        let source = page("<text>{user.na}</text>");
        let start = source.find("na}").unwrap();
        mesh_syntax::Span {
            start_byte: start,
            end_byte: start + 2,
        }
    });
}

#[test]
fn a_member_uses_analyze_expression_on_a_broken_file() {
    // `user.` is a syntax error: no canonical analysis. Even the whole
    // file collapsing (no `}`) still types `user`.
    let members = offered(&page("<text>{user.|}</text>"), "users-page");
    assert_eq!(labels(&members), ["active", "avatar", "name"]);
    assert_eq!(members[1].detail.as_deref(), Some("string?"));
    let members = offered("<page title=\"U\">\n  <text>{user.|\n</page>", "users-page");
    assert_eq!(labels(&members), ["active", "avatar", "name"]);
}

#[test]
fn nothing_is_offered_without_a_type_or_a_declared_tag() {
    assert!(offered(&page("<text>{usr.|}</text>"), "users-page").is_empty());
    assert!(offered(&page("<text>{user.avatar.|}</text>"), "users-page").is_empty());
    assert!(offered(&page("<nope |"), "users-page").is_empty());
    assert!(offered(&page("<text>hello |</text>"), "users-page").is_empty());
}
