//! A temporary workspace with the example manifest in it, shared by the
//! integration tests. Each test file uses some of it.
#![allow(dead_code)]

use mesh_lsp::testing::file_uri;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Workspace {
    dir: tempfile::TempDir,
}

impl Workspace {
    /// An empty workspace directory.
    pub fn empty() -> Workspace {
        Workspace {
            dir: tempfile::tempdir().expect("a temp dir"),
        }
    }

    /// A workspace holding `components.json`, a copy of the example
    /// manifest.
    pub fn with_example_manifest() -> Workspace {
        let workspace = Workspace::empty();
        workspace.write("components.json", &example("components.json"));
        workspace
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    pub fn uri(&self, relative: &str) -> String {
        file_uri(&self.path(relative))
    }

    pub fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a directory");
        }
        fs::write(&path, text).expect("a file");
        path
    }

    /// `initialize` parameters for this workspace, with `settings` as the
    /// `"mesh"` configuration and `capabilities` as the client's.
    pub fn init(&self, settings: Value, capabilities: Value) -> Value {
        json!({
            "processId": null,
            "rootUri": file_uri(self.root()),
            "capabilities": capabilities,
            "initializationOptions": settings,
        })
    }
}

/// A file from the repository's `examples/` directory.
pub fn example(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// The codes of `diagnostics`, in order.
pub fn codes(diagnostics: &[lsp_types::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| match &diagnostic.code {
            Some(lsp_types::NumberOrString::String(code)) => code.clone(),
            other => panic!("unexpected code {other:?}"),
        })
        .collect()
}
