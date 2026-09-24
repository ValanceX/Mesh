//! Hover and go-to-definition, from canonical state (outline, Pass 2).
//!
//! The agreement tests check the server against the compiler itself: at
//! every character of every template in the corpus, what hover and
//! definition answer is what `compile_with`'s analysis says is there.

mod common;

use common::{example, Workspace};
use mesh_analysis::{Analysis, Target};
use mesh_compiler::{ColumnUnit, CompileOptions, LineColumn, SourceMap};
use mesh_lsp::testing::Client;
use mesh_manifest::{Declaration, Manifest};
use mesh_syntax::Span;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// A server whose workspace holds `manifest` as `components.json`, with
/// each of `documents` (a relative path and a component) configured.
fn server(manifest: &str, documents: &[(&str, &str)], encoding: &str) -> (Workspace, Client) {
    let workspace = Workspace::empty();
    workspace.write("components.json", manifest);
    let components: serde_json::Map<String, Value> = documents
        .iter()
        .map(|(path, component)| (path.to_string(), json!(component)))
        .collect();
    let settings = json!({ "model": "components.json", "components": components });
    let capabilities = json!({
        "general": { "positionEncodings": [encoding] },
        "textDocument": { "hover": { "contentFormat": ["plaintext"] } }
    });
    let (client, _) = Client::initialized(workspace.init(settings, capabilities));
    (workspace, client)
}

fn unit(encoding: &str) -> ColumnUnit {
    match encoding {
        "utf-8" => ColumnUnit::Utf8,
        _ => ColumnUnit::Utf16,
    }
}

/// The LSP position of byte `offset` in `source`.
fn position(source: &str, offset: usize, unit: ColumnUnit) -> (u32, u32) {
    let at = SourceMap::new(source).line_column(source, offset, unit);
    (at.line as u32, at.column as u32)
}

/// The byte span of an LSP range in `source`.
fn span_of_range(source: &str, range: &Value, unit: ColumnUnit) -> Span {
    let map = SourceMap::new(source);
    let offset = |at: &Value| {
        map.offset(
            source,
            LineColumn {
                line: at["line"].as_u64().unwrap() as usize,
                column: at["character"].as_u64().unwrap() as usize,
            },
            unit,
        )
    };
    Span {
        start_byte: offset(&range["start"]),
        end_byte: offset(&range["end"]),
    }
}

fn hover_value(result: &Value) -> &str {
    result["contents"]["value"]
        .as_str()
        .expect("a markup hover")
}

/// The byte offset of the `nth` character of the first `needle`.
fn at(source: &str, needle: &str, nth: usize) -> usize {
    source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found"))
        + nth
}

// --- The Definition of Done's navigation items ------------------------

fn users_page() -> (Workspace, Client, String, String) {
    let (workspace, mut client) = server(
        &example("components.json"),
        &[("users-page.mprx", "users-page")],
        "utf-16",
    );
    let uri = workspace.uri("users-page.mprx");
    let source = example("users-page.mprx");
    client.open(&uri, 1, &source);
    client.next_publication(&uri);
    (workspace, client, uri, source)
}

fn hover(client: &mut Client, uri: &str, source: &str, offset: usize) -> Value {
    let (line, character) = position(source, offset, ColumnUnit::Utf16);
    client.hover(uri, line, character)
}

fn definition(client: &mut Client, uri: &str, source: &str, offset: usize) -> Value {
    let (line, character) = position(source, offset, ColumnUnit::Utf16);
    client.definition(uri, line, character)
}

#[test]
fn hovering_a_member_and_a_scope_name_shows_their_types() {
    let (_workspace, mut client, uri, source) = users_page();
    let name = hover(&mut client, &uri, &source, at(&source, "user.name", 6));
    assert_eq!(hover_value(&name), "string");
    let compact = hover(&mut client, &uri, &source, at(&source, "compact ?", 2));
    assert_eq!(
        hover_value(&compact),
        "compact: boolean\n\nIn the scope of `users-page`."
    );
}

#[test]
fn definitions_land_on_their_manifest_keys() {
    let (workspace, mut client, uri, source) = users_page();
    let manifest = example("components.json");
    for (needle, nth, key) in [
        ("<avatar", 2, r#""avatar""#),
        ("alt=", 1, r#""alt""#),
        ("selectUser", 3, r#""selectUser""#),
    ] {
        let location = definition(&mut client, &uri, &source, at(&source, needle, nth));
        assert_eq!(location["uri"], json!(workspace.uri("components.json")));
        let span = span_of_range(&manifest, &location["range"], ColumnUnit::Utf16);
        assert_eq!(&manifest[span.start_byte..span.end_byte], key, "{needle}");
        // The prop's key is the one inside "avatar", not the first "alt".
        if key == r#""alt""# {
            assert!(span.start_byte > manifest.find(r#""avatar": {"#).unwrap());
        }
    }
}

#[test]
fn a_definition_while_the_manifest_is_open_uses_its_buffer() {
    let (workspace, mut client, uri, source) = users_page();
    let manifest =
        example("components.json").replacen("\"components\": {", "\"components\": {\n", 1);
    client.open(&workspace.uri("components.json"), 1, &manifest);
    client.latest_publication(&uri, std::time::Duration::from_millis(200));
    let location = definition(&mut client, &uri, &source, at(&source, "<avatar", 2));
    let span = span_of_range(&manifest, &location["range"], ColumnUnit::Utf16);
    assert_eq!(&manifest[span.start_byte..span.end_byte], r#""avatar""#);
}

#[test]
fn no_hover_on_a_file_with_a_syntax_error() {
    let (_workspace, mut client, uri, source) = users_page();
    let broken = source.replace("</page>", "</page");
    client.change(&uri, 2, &broken);
    client.next_publication(&uri);
    assert_eq!(
        hover(&mut client, &uri, &broken, at(&broken, "compact ?", 2)),
        Value::Null
    );
}

#[test]
fn no_hover_without_a_model() {
    let (workspace, mut client) = server(&example("components.json"), &[], "utf-16");
    let uri = workspace.uri("users-page.mprx");
    let source = example("users-page.mprx");
    client.open(&uri, 1, &source);
    client.next_publication(&uri);
    assert_eq!(
        hover(&mut client, &uri, &source, at(&source, "compact ?", 2)),
        Value::Null
    );
    assert_eq!(
        definition(&mut client, &uri, &source, at(&source, "<avatar", 2)),
        Value::Null
    );
}

#[test]
fn no_definition_for_a_closing_tag_or_a_member() {
    let (_workspace, mut client, uri, source) = users_page();
    let closing = at(&source, "</button", 3);
    assert_eq!(definition(&mut client, &uri, &source, closing), Value::Null);
    let member = at(&source, "user.name", 6);
    assert_eq!(definition(&mut client, &uri, &source, member), Value::Null);
}

// --- Agreement with analysis, everywhere --------------------------------

/// Every template in the corpus with a model: a name, its text, the
/// manifest it's checked against, and its component, as the CLI's corpus
/// test checks them. Plus the users page with characters outside the
/// Basic Multilingual Plane before names that resolve, so the column
/// units differ where there is something to find.
fn corpus() -> Vec<(String, String, String, String)> {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let read = |path: &Path| fs::read_to_string(path).unwrap();
    let example_manifest = read(&examples.join("components.json"));
    let check_manifest = read(&examples.join("fixtures/check/components.json"));
    let mut runs = Vec::new();
    for (file, component) in [
        ("users-page.mprx", "users-page"),
        ("user-card.mprx", "user-card-example"),
        ("page.mprx", "page"),
    ] {
        runs.push((
            file.to_string(),
            read(&examples.join(file)),
            example_manifest.clone(),
            component.to_string(),
        ));
    }
    runs.push((
        "users-page.mprx, with emoji".to_string(),
        read(&examples.join("users-page.mprx"))
            .replace("<text>{user.name}", "<text>😀 é {user.name} {compact}"),
        example_manifest.clone(),
        "users-page".to_string(),
    ));
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        let mut files: Vec<PathBuf> = fs::read_dir(examples.join(dir))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "mprx"))
            .collect();
        files.sort();
        for file in files {
            runs.push((
                file.display().to_string(),
                read(&file),
                check_manifest.clone(),
                "template".to_string(),
            ));
        }
    }
    runs
}

/// What the server should answer at `offset`, by the compiler's analysis
/// (Pass 2, Decision 7): the hovered span, the type a type hover shows,
/// and the declaration definition goes to.
struct Expected<'a> {
    hover: Option<(Span, Option<String>)>,
    definition: Option<Declaration<'a>>,
}

fn expected<'a>(analysis: &'a Analysis, component: &'a str, offset: usize) -> Expected<'a> {
    let resolution = analysis.resolution_at(offset);
    let typed = analysis.typed_at(offset);
    let length = |span: Span| span.end_byte - span.start_byte;
    let hover = match (resolution, typed) {
        (Some(r), Some(t)) if length(r.span) <= length(t.span) => Some((r.span, None)),
        (Some(r), None) => Some((r.span, None)),
        (_, Some(t)) => Some((t.span, Some(t.ty.to_string()))),
        (None, None) => None,
    };
    let definition = resolution.map(|resolution| match &resolution.target {
        Target::Component(name) => Declaration::Component(name),
        Target::Prop { component, prop } => Declaration::Prop { component, prop },
        Target::Event { component, event } => Declaration::Event { component, event },
        Target::Scope(name) => Declaration::Scope { component, name },
        Target::Command(command) => Declaration::Command { component, command },
    });
    Expected { hover, definition }
}

fn navigation_agrees_with_analysis(encoding: &str) {
    let unit = unit(encoding);
    let mut checked = 0;
    for (name, source, manifest_text, component) in corpus() {
        let manifest: Manifest = mesh_manifest::load(&manifest_text).unwrap();
        let template = manifest.template(&component).unwrap();
        let result = mesh_compiler::compile_with(&source, &CompileOptions::with_template(template));
        let Some(analysis) = &result.analysis else {
            assert!(!name.contains("emoji"), "the emoji run must parse");
            continue;
        };
        let (workspace, mut client) =
            server(&manifest_text, &[("t.mprx", component.as_str())], encoding);
        let uri = workspace.uri("t.mprx");
        client.open(&uri, 1, &source);
        client.next_publication(&uri);
        for (offset, _) in source.char_indices().chain([(source.len(), ' ')]) {
            let (line, character) = position(&source, offset, unit);
            let want = expected(analysis, &component, offset);

            let hover = client.hover(&uri, line, character);
            match &want.hover {
                None => assert_eq!(hover, Value::Null, "{name} at {offset}"),
                Some((span, ty)) => {
                    let got = span_of_range(&source, &hover["range"], unit);
                    assert_eq!(got, *span, "{name}: hover range at {offset}");
                    let value = hover_value(&hover);
                    match ty {
                        // A type hover is the type, and a named type's
                        // definition, with no sentence.
                        Some(ty) => {
                            assert_eq!(
                                value.lines().next(),
                                Some(ty.as_str()),
                                "{name} at {offset}"
                            );
                            assert!(!value.contains("\n\n"), "{name} at {offset}: {value}");
                        }
                        // A declaration hover names what it declares.
                        None => assert!(
                            value.starts_with("component ") || value.contains("\n\n"),
                            "{name} at {offset}: {value}"
                        ),
                    }
                }
            }

            let location = client.definition(&uri, line, character);
            match want
                .definition
                .and_then(|declaration| manifest.span_of(declaration))
            {
                None => assert_eq!(location, Value::Null, "{name} at {offset}"),
                Some(span) => {
                    let got = span_of_range(&manifest_text, &location["range"], unit);
                    assert_eq!(got, span, "{name}: definition at {offset}");
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} offsets checked");
}

#[test]
fn navigation_agrees_with_analysis_in_utf16() {
    navigation_agrees_with_analysis("utf-16");
}

#[test]
fn navigation_agrees_with_analysis_in_utf8() {
    navigation_agrees_with_analysis("utf-8");
}
