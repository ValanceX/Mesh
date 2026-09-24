//! The npm package family's names (outline D11): every MESH package is
//! scoped `@valance/`, and so is every dependency between them.

use std::fs;
use std::path::Path;

/// Every `packages/*/package.json`, parsed.
fn packages() -> Vec<(String, serde_json::Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    let mut found = Vec::new();
    for entry in fs::read_dir(&root).expect("should read packages/") {
        let path = entry
            .expect("a directory entry")
            .path()
            .join("package.json");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("should read {}: {err}", path.display()));
        let json = serde_json::from_str(&text).expect("package.json is JSON");
        found.push((path.display().to_string(), json));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

#[test]
fn every_package_is_scoped_valance() {
    let packages = packages();
    assert!(
        packages.len() >= 3,
        "only {} packages found",
        packages.len()
    );
    for (path, json) in packages {
        let name = json["name"].as_str().expect("a package has a name");
        assert!(name.starts_with("@valance/"), "{path}: {name}");
        assert_ne!(name, "mesh", "{path}");
    }
}

#[test]
fn packages_depend_on_each_other_by_their_scoped_names() {
    for (path, json) in packages() {
        let Some(dependencies) = json["dependencies"].as_object() else {
            continue;
        };
        for name in dependencies.keys() {
            // Any dependency on a MESH package names it by its scope.
            if name.contains("mesh") {
                assert!(name.starts_with("@valance/"), "{path}: {name}");
            }
        }
    }
}
