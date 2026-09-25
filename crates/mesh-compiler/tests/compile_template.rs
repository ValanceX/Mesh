//! Compiling (v0.5 D2): a source checked clean as a component's template
//! compiles to `template-v1`; a source with errors compiles to nothing
//! and reports exactly what the check reports.

use mesh_compiler::check::{self, Model};
use mesh_template::{Child, Expression, Literal, Template};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// Every `.mprx` directly in `dir`, sorted.
fn sources(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(root().join(dir))
        .unwrap_or_else(|err| panic!("should list {dir}: {err}"))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mprx"))
        .collect();
    files.sort();
    files
}

fn model(manifest: &str, component: &str) -> Model {
    Model::load(&read(&root().join(manifest)), component)
        .unwrap_or_else(|diagnostics| panic!("{manifest} loads: {diagnostics:#?}"))
}

/// Every corpus run with a model: (source path, its text, the model).
fn corpus() -> Vec<(PathBuf, String, Model)> {
    let mut runs = Vec::new();
    for path in sources("examples") {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        // `user-card.mprx` is a usage snippet, the template of
        // `user-card-example` (as `crates/mesh-cli/tests/fixtures.rs` says).
        let component = if stem == "user-card" {
            "user-card-example".into()
        } else {
            stem
        };
        runs.push((
            path.clone(),
            read(&path),
            model("examples/components.json", &component),
        ));
    }
    for dir in [
        "examples/fixtures/check/pass",
        "examples/fixtures/check/fail",
    ] {
        for path in sources(dir) {
            let text = read(&path);
            runs.push((
                path,
                text,
                model("examples/fixtures/check/components.json", "template"),
            ));
        }
    }
    runs
}

fn validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(&read(&root().join("schemas/template-v1.schema.json"))).unwrap();
    jsonschema::draft202012::new(&schema).expect("the schema is valid")
}

#[test]
fn every_clean_source_compiles_to_a_valid_template() {
    let validator = validator();
    let mut compiled = 0;
    for (path, text, model) in corpus() {
        let result = check::template(&text, &model);
        assert_eq!(
            result.diagnostics,
            check::source(&text, Some(&model)),
            "{}: compiling reports exactly what checking reports",
            path.display()
        );
        if check::has_errors(&result.diagnostics) {
            assert!(
                result.template.is_none(),
                "{}: no template from errors",
                path.display()
            );
            continue;
        }
        let template = result
            .template
            .unwrap_or_else(|| panic!("{}: a clean source compiles", path.display()));
        assert_eq!(template.component, model.component());
        assert_eq!(template.fingerprint, model.fingerprint());
        let written: Value = serde_json::from_str(&mesh_template::to_json(&template)).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&written)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{}: {errors:#?}", path.display());
        assert_eq!(
            mesh_template::from_json(&mesh_template::to_json(&template)).as_ref(),
            Ok(&template),
            "{}: it reads back",
            path.display()
        );
        compiled += 1;
    }
    assert!(compiled >= 20, "only {compiled} clean sources compiled");
}

#[test]
fn every_source_with_errors_compiles_to_nothing() {
    let mut refused = 0;
    for (path, text, model) in corpus() {
        // By the directory's name, not a `/fail/` substring, which a
        // Windows path spells `\fail\`.
        if path.parent().and_then(|dir| dir.file_name()) == Some("fail".as_ref()) {
            let result = check::template(&text, &model);
            assert!(check::has_errors(&result.diagnostics), "{}", path.display());
            assert!(result.template.is_none(), "{}", path.display());
            refused += 1;
        }
    }
    assert!(refused >= 20);
}

#[test]
fn warnings_do_not_stop_it() {
    let model = model("examples/fixtures/check/components.json", "template");
    let source = r#"<probe str="first" str={name} />"#;
    let result = check::template(source, &model);
    assert_eq!(
        result.diagnostics.len(),
        1,
        "one duplicate-attribute warning"
    );
    assert!(!check::has_errors(&result.diagnostics));
    let template = result.template.expect("warnings don't stop compiling");
    assert_eq!(template.root.props.len(), 1, "the last attribute is kept");
    assert!(matches!(
        template.root.props[0].value,
        Expression::Scope { .. }
    ));
}

#[test]
fn compiling_is_deterministic() {
    for (path, text, model) in corpus() {
        let first = check::template(&text, &model)
            .template
            .map(|t| mesh_template::to_json(&t));
        let second = check::template(&text, &model)
            .template
            .map(|t| mesh_template::to_json(&t));
        assert_eq!(first, second, "{}", path.display());
    }
}

/// The literal a one-prop template compiles its `num` prop to.
fn number(literal: &str) -> f64 {
    let model = model("examples/fixtures/check/components.json", "template");
    let template = check::template(&format!("<probe num={{{literal}}} />"), &model)
        .template
        .unwrap_or_else(|| panic!("{literal} compiles"));
    match &template.root.props[0].value {
        Expression::Literal {
            value: Literal::Number(value),
            ..
        } => *value,
        other => panic!("{literal}: {other:?}"),
    }
}

#[test]
fn literals_are_their_binary64_values() {
    assert_eq!(number("1").to_bits(), 1f64.to_bits());
    assert_eq!(number("1.0").to_bits(), number("1").to_bits());
    assert_eq!(number("1.00").to_bits(), number("1").to_bits());
    assert_eq!(number("1.50").to_bits(), number("1.5").to_bits());
    assert_eq!(number("0.1").to_bits(), 0.1f64.to_bits());
    // A tie rounds to even: 2^53 + 1 is halfway between 2^53 and 2^53 + 2.
    assert_eq!(number("9007199254740993"), 9_007_199_254_740_992.0);
    assert_eq!(number("9007199254740995"), 9_007_199_254_740_996.0);
}

/// Spans count bytes and UTF-16 code units of the source as given.
#[test]
fn spans_have_byte_and_utf16_offsets() {
    let model = model("examples/fixtures/check/components.json", "template");
    let source = "<text>é😀 {name}</text>";
    let template = check::template(source, &model)
        .template
        .expect("it compiles");
    let Child::Expression { expression } = &template.root.children[1] else {
        panic!("{:?}", template.root.children);
    };
    let span = expression.span();
    assert_eq!(span.start.byte, source.find("name").unwrap());
    assert_eq!(span.start.utf16, "<text>é😀 {".encode_utf16().count());
    assert_eq!(span.end.utf16 - span.start.utf16, 4);
}

#[test]
fn users_page_compiles_to_the_committed_template() {
    let path = root().join("examples/users-page.mprx");
    let template = check::template(
        &read(&path),
        &model("examples/components.json", "users-page"),
    )
    .template
    .expect("users-page compiles");
    let expected = root().join("crates/mesh-compiler/tests/expected/users-page.template.json");
    // The compiler version is provenance only; the committed file says
    // 0.0.0, so it doesn't change with every release.
    let template = Template {
        compiler: "0.0.0".into(),
        ..template
    };
    let written = mesh_template::to_json(&template);
    if std::env::var_os("MESH_BLESS").is_some() {
        let pretty: Value = serde_json::from_str(&written).unwrap();
        fs::write(
            &expected,
            serde_json::to_string_pretty(&pretty).unwrap() + "\n",
        )
        .unwrap();
    }
    let expected: Template =
        mesh_template::from_json(&read(&expected)).expect("the expected template reads");
    assert_eq!(template, expected);
}

/// Every name a template holds is a declaration a name resolved to: the
/// template's names, as a multiset, are the resolutions' targets.
#[test]
fn a_templates_names_are_its_resolutions() {
    use mesh_compiler::{compile_with, CompileOptions};
    for (path, text, model) in corpus() {
        let Some(template) = check::template(&text, &model).template else {
            continue;
        };
        let manifest = mesh_manifest::load(&read(&root().join(
            if path.to_string_lossy().contains("fixtures/check") {
                "examples/fixtures/check/components.json"
            } else {
                "examples/components.json"
            },
        )))
        .unwrap();
        let options = CompileOptions::with_template(manifest.template(model.component()).unwrap());
        let analysis = compile_with(&text, &options).analysis.expect("analysed");
        let mut resolved: Vec<String> = analysis
            .resolutions()
            .iter()
            .map(|resolution| format!("{:?}", resolution.target))
            .collect();
        let mut held = Vec::new();
        names(&template.root, &mut held);
        resolved.sort();
        held.sort();
        assert_eq!(held, resolved, "{}", path.display());
    }
}

fn names(element: &mesh_template::Element, out: &mut Vec<String>) {
    use mesh_analysis::Target;
    let component = &element.component;
    out.push(format!("{:?}", Target::Component(component.clone())));
    for prop in &element.props {
        out.push(format!(
            "{:?}",
            Target::Prop {
                component: component.clone(),
                prop: prop.prop.clone()
            }
        ));
        expression_names(&prop.value, out);
    }
    for binding in &element.events {
        out.push(format!(
            "{:?}",
            Target::Event {
                component: component.clone(),
                event: binding.event.clone()
            }
        ));
        out.push(format!("{:?}", Target::Command(binding.command.clone())));
        for argument in &binding.arguments {
            expression_names(argument, out);
        }
    }
    for child in &element.children {
        match child {
            Child::Text { .. } => {}
            Child::Expression { expression } => expression_names(expression, out),
            Child::Element { element } => names(element, out),
        }
    }
}

fn expression_names(expression: &Expression, out: &mut Vec<String>) {
    use mesh_analysis::Target;
    match expression {
        Expression::Scope { name, .. } => out.push(format!("{:?}", Target::Scope(name.clone()))),
        Expression::Literal { .. } | Expression::Event { .. } => {}
        Expression::Member { object, .. } => expression_names(object, out),
        Expression::Unary { operand, .. } => expression_names(operand, out),
        Expression::Binary { left, right, .. } => {
            expression_names(left, out);
            expression_names(right, out);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            expression_names(condition, out);
            expression_names(consequent, out);
            expression_names(alternate, out);
        }
        Expression::List { elements, .. } => elements.iter().for_each(|e| expression_names(e, out)),
        Expression::Record { fields, .. } => {
            fields
                .iter()
                .for_each(|field| expression_names(&field.value, out));
        }
    }
}
