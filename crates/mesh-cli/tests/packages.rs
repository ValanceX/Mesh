//! The npm package family (outline v0.4 D7): every MESH package is scoped
//! `@valancex/`, the family is exactly the packages listed here, and each
//! is versioned in lockstep with the Rust workspace, as is every
//! dependency between them.

use std::fs;
use std::path::{Path, PathBuf};

/// Every package v0.4 has, by name.
const PACKAGES: [&str; 2] = ["@valancex/mesh-compiler", "@valancex/mesh-lsp"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `packages/*/package.json`, parsed.
fn packages() -> Vec<(String, serde_json::Value)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root().join("packages")).expect("should read packages/") {
        let path = entry.expect("a directory entry").path();
        if !path.is_dir() || path.file_name().is_some_and(|name| name == "node_modules") {
            continue;
        }
        let path = path.join("package.json");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("should read {}: {err}", path.display()));
        let json = serde_json::from_str(&text).expect("package.json is JSON");
        found.push((path.display().to_string(), json));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// `[workspace.package] version` in the root `Cargo.toml`.
fn workspace_version() -> String {
    let manifest = fs::read_to_string(root().join("Cargo.toml")).expect("should read Cargo.toml");
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
        } else if in_package {
            if let Some(value) = line.strip_prefix("version = ") {
                return value.trim_matches('"').to_string();
            }
        }
    }
    panic!("Cargo.toml has no [workspace.package] version");
}

fn name(json: &serde_json::Value) -> &str {
    json["name"].as_str().expect("a package has a name")
}

#[test]
fn the_family_is_exactly_the_listed_packages() {
    let mut names: Vec<String> = packages()
        .iter()
        .map(|(_, json)| name(json).to_string())
        .collect();
    names.sort();
    assert_eq!(names, PACKAGES);
}

#[test]
fn every_package_is_scoped_valancex() {
    for (path, json) in packages() {
        let name = name(&json);
        assert!(name.starts_with("@valancex/"), "{path}: {name}");
        assert_ne!(name, "mesh", "{path}");
    }
}

#[test]
fn every_package_has_the_workspace_version() {
    let version = workspace_version();
    for (path, json) in packages() {
        assert_eq!(json["version"], version.as_str(), "{path}");
    }
}

#[test]
fn packages_depend_on_each_other_by_scope_at_the_workspace_version() {
    let version = workspace_version();
    for (path, json) in packages() {
        for field in ["dependencies", "optionalDependencies", "peerDependencies"] {
            let Some(dependencies) = json[field].as_object() else {
                continue;
            };
            for (name, wanted) in dependencies {
                if name.contains("mesh") {
                    assert!(name.starts_with("@valancex/"), "{path}: {name}");
                    assert_eq!(wanted, version.as_str(), "{path}: {name}");
                }
            }
        }
    }
}

/// The compiler package's `version.ts` is the version its wrapper expects
/// the WebAssembly module to report (outline v0.4 I10), so it too is the
/// workspace's, which is the module's.
#[test]
fn the_compiler_wrapper_expects_the_workspace_version() {
    let path = root().join("packages/mesh-compiler/src/version.ts");
    let text = fs::read_to_string(&path).expect("should read version.ts");
    let expected = format!("export const version = \"{}\";", workspace_version());
    assert!(
        text.lines().any(|line| line.trim() == expected),
        "{} should declare {expected}",
        path.display()
    );
}
