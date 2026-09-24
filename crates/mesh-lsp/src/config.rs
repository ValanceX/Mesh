//! The server's configuration (outline D6): which manifest to check
//! against, and which component each document is the template of.
//!
//! The same object arrives as `initializationOptions`, as the answer to a
//! `workspace/configuration` request for section `"mesh"`, or as
//! `settings.mesh` in `workspace/didChangeConfiguration`:
//!
//! ```json
//! { "model": "components.json",
//!   "components": { "users-page.mprx": "users-page" } }
//! ```
//!
//! Nothing here derives a component from a file name (invariant I5): a
//! document the map doesn't name is checked without a model.

use crate::uri;
use serde_json::Value;
use std::collections::BTreeMap;

/// The parsed configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Config {
    /// The manifest, as written: relative to the workspace root, or
    /// absolute.
    pub(crate) model: Option<String>,
    /// Workspace-relative document key (see [`uri::key`]) → component.
    pub(crate) components: BTreeMap<String, String>,
}

impl Config {
    /// Reads the `"mesh"` settings object. A setting of the wrong type is
    /// left out, and each problem is returned as a warning to log; nothing
    /// in a configuration can stop the server.
    pub(crate) fn parse(value: &Value) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut warnings = Vec::new();
        let object = match value {
            Value::Object(object) => object,
            Value::Null => return (config, warnings),
            other => {
                warnings.push(format!(
                    "the \"mesh\" settings must be an object, not {}",
                    describe(other)
                ));
                return (config, warnings);
            }
        };

        match object.get("model") {
            None | Some(Value::Null) => {}
            Some(Value::String(model)) => config.model = Some(model.clone()),
            Some(other) => warnings.push(format!(
                "\"mesh.model\" must be a path, not {}; checking without a model",
                describe(other)
            )),
        }

        match object.get("components") {
            None | Some(Value::Null) => {}
            Some(Value::Object(components)) => {
                for (written, component) in components {
                    let Some(key) = uri::normalize_key(written) else {
                        warnings.push(format!(
                            "\"mesh.components\" key {written:?} isn't a path inside the workspace; ignoring it"
                        ));
                        continue;
                    };
                    match component {
                        Value::String(component) => {
                            config.components.insert(key, component.clone());
                        }
                        other => warnings.push(format!(
                            "\"mesh.components\" value for {written:?} must be a component name, not {}; ignoring it",
                            describe(other)
                        )),
                    }
                }
            }
            Some(other) => warnings.push(format!(
                "\"mesh.components\" must be an object, not {}",
                describe(other)
            )),
        }

        (config, warnings)
    }
}

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_a_model_and_components() {
        let (config, warnings) = Config::parse(&json!({
            "model": "components.json",
            "components": { "./pages/home.mprx": "home", "a.mprx": "a" }
        }));
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(config.model.as_deref(), Some("components.json"));
        assert_eq!(config.components["pages/home.mprx"], "home");
        assert_eq!(config.components["a.mprx"], "a");
    }

    #[test]
    fn nothing_configured_is_empty() {
        assert_eq!(Config::parse(&Value::Null), (Config::default(), vec![]));
        assert_eq!(Config::parse(&json!({})), (Config::default(), vec![]));
    }

    #[test]
    fn wrong_types_are_left_out_one_by_one() {
        let (config, warnings) = Config::parse(&json!({
            "model": 3,
            "components": { "a.mprx": "a", "b.mprx": false, "../c.mprx": "c" }
        }));
        assert_eq!(config.model, None);
        assert_eq!(config.components.len(), 1);
        assert_eq!(warnings.len(), 3, "{warnings:#?}");
        let (config, warnings) = Config::parse(&json!("components.json"));
        assert_eq!(config, Config::default());
        assert_eq!(warnings.len(), 1);
    }
}
