//! Writing a template.

use crate::types::{Element, Template};
use crate::version::needed;
use crate::FORMAT;
use serde::Serialize;

/// The document as written, in the schema's property order.
#[derive(Serialize)]
struct Document<'a> {
    format: &'static str,
    version: u64,
    component: &'a str,
    fingerprint: String,
    compiler: &'a str,
    root: &'a Element,
}

/// Writes `template` as a `template-v1` document, or a `template-v2` one if it uses a construct
/// only version 2 has: compact (no
/// whitespace), with properties in the schema's order, so the same
/// template always gives the same bytes. Every number is written so that
/// it reads back as exactly the same binary64 value.
pub fn to_json(template: &Template) -> String {
    serde_json::to_string(&Document {
        format: FORMAT,
        version: needed(&template.root),
        component: &template.component,
        fingerprint: template.fingerprint.to_string(),
        compiler: &template.compiler,
        root: &template.root,
    })
    .expect("a template always serializes")
}
