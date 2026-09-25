//! The test host (D10): supplies snapshots from JSON fixtures, keeps
//! every render, relays each event to dispatch with the render it names,
//! and records what dispatch gives. It models the host's obligation
//! (docs/manual/runtime.md): an event is dispatched with the render whose
//! tree the renderer had drawn.

use mesh_runtime::{HostRecord, HostValue, Program, Render, RuntimeDiagnostic};

pub struct Host {
    pub model: String,
    pub root: String,
    pub templates: Vec<String>,
    /// Every render, in order: the host keeps them all.
    pub renders: Vec<Render>,
}

impl Host {
    /// Renders from a snapshot fixture, keeping the render.
    pub fn render(&mut self, snapshot: &str) -> Result<&Render, Vec<RuntimeDiagnostic>> {
        let record = HostRecord::from_json(snapshot).expect("the fixture is JSON");
        let texts: Vec<&str> = self.templates.iter().map(String::as_str).collect();
        let render = mesh_runtime::render(
            &Program {
                root: &self.root,
                templates: &texts,
            },
            &self.model,
            &record,
        )?;
        self.renders.push(render);
        Ok(self.renders.last().unwrap())
    }

    /// Dispatches `event` of the `occurrence`th node binding it (document
    /// order) in render `render`, with that render, and `payload`. The
    /// result as the document a host would record: the intent, or the
    /// runtime diagnostics document.
    pub fn dispatch(
        &self,
        render: usize,
        event: &str,
        occurrence: usize,
        payload: Option<&str>,
    ) -> String {
        let kept = &self.renders[render];
        let mut handlers = Vec::new();
        collect(&kept.tree().root, event, &mut handlers);
        let handler = &handlers[occurrence];
        let payload = payload.map(|text| HostValue::from_json(text).expect("the payload is JSON"));
        match mesh_runtime::dispatch(kept, handler, payload.as_ref()) {
            Ok(intent) => intent.to_json(),
            Err(diagnostics) => mesh_runtime::to_json(&diagnostics, &self.model),
        }
    }
}

fn collect(node: &mesh_runtime::Node, event: &str, out: &mut Vec<String>) {
    if let Some(handler) = node.events.get(event) {
        out.push(handler.clone());
    }
    for child in &node.children {
        if let mesh_runtime::TreeChild::Node(child) = child {
            collect(child, event, out);
        }
    }
}
