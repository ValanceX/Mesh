#[test]
fn compiles_valid_source_with_no_diagnostics() {
    let result = mesh_compiler::compile(r#"<page title="Users" />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(ir.name, "page");
    assert_eq!(ir.attributes[0].name, "title");
    assert_eq!(ir.attributes[0].value, "Users");
}

#[test]
fn compiles_container_element_and_preserves_text_content() {
    let result = mesh_compiler::compile("<title>Users</title>");

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(ir.name, "title");
    assert_eq!(ir.children.len(), 1);
    assert_eq!(ir.children[0].value, "Users");
}

#[test]
fn compiling_invalid_source_produces_an_error_diagnostic() {
    let result = mesh_compiler::compile("<page");

    assert!(result.ir.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].severity,
        mesh_compiler::Severity::Error
    );
}
