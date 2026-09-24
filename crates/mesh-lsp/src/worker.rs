//! The compile thread. The message loop sends it one job per snapshot and
//! it sends back what the canonical compiler said (outline D10).
//!
//! It calls `mesh_compiler::compile_with` or `compile` and nothing else
//! (invariant I1): the diagnostics it returns are exactly the compiler's.

use crossbeam_channel::{unbounded, Receiver, Sender};
use lsp_server::RequestId;
use mesh_compiler::editor::{self, Candidate, Recovery};
use mesh_compiler::{CompileOptions, CompileResult};
use mesh_manifest::Manifest;
use std::any::Any;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::thread;

/// The compile thread's stack. The nesting tests pin the whole pipeline,
/// unoptimized, to 2 MiB; this is four times that (outline D7).
const STACK: usize = 8 * 1024 * 1024;

/// Which snapshot a result belongs to. A result is published only if its
/// identity is still the document's current one (outline D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity {
    /// The server's own count of accepted `didOpen`/`didChange`s.
    pub(crate) document: u64,
    /// The manifest snapshot's generation.
    pub(crate) manifest: u64,
    /// The configuration's generation.
    pub(crate) config: u64,
}

/// What a document is checked against.
#[derive(Clone)]
pub(crate) enum Model {
    /// Nothing: syntax and structure only, as `mesh check` without
    /// `--model`.
    None,
    /// The template of `component`, which the manifest declares.
    Template {
        manifest: Arc<Manifest>,
        component: String,
    },
}

pub(crate) struct Job {
    pub(crate) uri: String,
    pub(crate) identity: Identity,
    /// The document's LSP version, published with the result.
    pub(crate) version: i32,
    pub(crate) text: Arc<str>,
    pub(crate) model: Model,
}

pub(crate) struct Done {
    pub(crate) uri: String,
    pub(crate) identity: Identity,
    pub(crate) version: i32,
    pub(crate) text: Arc<str>,
    /// What the snapshot was checked against.
    pub(crate) model: Model,
    /// The canonical compile's result and, for a snapshot with no IR
    /// checked against a model, its editor recovery (outline D3); or the
    /// message of a panic while compiling.
    pub(crate) result: Result<Compiled, String>,
}

/// What the compile thread found about one snapshot.
pub(crate) struct Compiled {
    /// Exactly what `compile_with` (or `compile`) returned.
    pub(crate) canonical: Arc<CompileResult>,
    /// Editor-only: what the parts that parse mean, when the canonical
    /// result has no IR. Never published, never merged into `canonical`.
    pub(crate) recovery: Option<Arc<Recovery>>,
}

/// A completion request, once its snapshot is compiled (outline D4).
/// Completion parses and may analyze, so it runs here, on the thread
/// whose stack is known (D7), never on the message loop.
pub(crate) struct CompleteJob {
    pub(crate) id: RequestId,
    pub(crate) uri: String,
    pub(crate) identity: Identity,
    pub(crate) text: Arc<str>,
    pub(crate) offset: usize,
    pub(crate) manifest: Arc<Manifest>,
    pub(crate) component: String,
    /// The snapshot's canonical result, whose analysis types a member's
    /// object when there is one.
    pub(crate) canonical: Arc<CompileResult>,
}

pub(crate) struct CompleteDone {
    pub(crate) id: RequestId,
    pub(crate) uri: String,
    pub(crate) identity: Identity,
    /// The candidates, or the message of a panic while finding them.
    pub(crate) candidates: Result<Vec<Candidate>, String>,
}

enum Task {
    Compile(Job),
    Complete(CompleteJob),
}

/// What the thread sends back.
pub(crate) enum Finished {
    Compiled(Done),
    Completed(CompleteDone),
}

pub(crate) struct Worker {
    jobs: Sender<Task>,
    pub(crate) results: Receiver<Finished>,
}

impl Worker {
    /// Queues `job`. Returns `false` if the thread is gone, which only
    /// happens if it couldn't keep running at all.
    pub(crate) fn send(&self, job: Job) -> bool {
        self.jobs.send(Task::Compile(job)).is_ok()
    }

    /// Queues a completion. Returns `false` if the thread is gone.
    pub(crate) fn complete(&self, job: CompleteJob) -> bool {
        self.jobs.send(Task::Complete(job)).is_ok()
    }
}

/// Starts the compile thread. With a `gate`, the thread waits for one
/// message on it before each compile, so a test can hold a compile back.
pub(crate) fn spawn(gate: Option<Receiver<()>>) -> io::Result<Worker> {
    let (jobs, job_receiver) = unbounded::<Task>();
    let (result_sender, results) = unbounded::<Finished>();
    thread::Builder::new()
        .name("mesh-lsp compile".to_string())
        .stack_size(STACK)
        .spawn(move || {
            for task in job_receiver {
                let job = match task {
                    Task::Compile(job) => job,
                    Task::Complete(job) => {
                        let candidates = panic::catch_unwind(AssertUnwindSafe(|| complete(&job)))
                            .map_err(|payload| panic_message(payload.as_ref()));
                        let done = CompleteDone {
                            id: job.id,
                            uri: job.uri,
                            identity: job.identity,
                            candidates,
                        };
                        if result_sender.send(Finished::Completed(done)).is_err() {
                            break;
                        }
                        continue;
                    }
                };
                if let Some(gate) = &gate {
                    // A dropped gate just stops holding compiles back.
                    let _ = gate.recv();
                }
                let result = panic::catch_unwind(AssertUnwindSafe(|| compile(&job)))
                    .map_err(|payload| panic_message(payload.as_ref()));
                let done = Done {
                    uri: job.uri,
                    identity: job.identity,
                    version: job.version,
                    text: job.text,
                    model: job.model,
                    result,
                };
                if result_sender.send(Finished::Compiled(done)).is_err() {
                    // The server has stopped.
                    break;
                }
            }
        })?;
    Ok(Worker { jobs, results })
}

/// The completion candidates at `job`'s offset (outline D4).
fn complete(job: &CompleteJob) -> Vec<Candidate> {
    match job.manifest.template(&job.component) {
        Ok(template) => editor::candidates(
            &job.text,
            job.offset,
            template,
            job.canonical.analysis.as_ref(),
        ),
        Err(_) => Vec::new(),
    }
}

/// The canonical compile of `job`'s snapshot, then, only if it has no IR
/// and a template, the editor recovery of the same text (recovery spec,
/// R1 and R8).
fn compile(job: &Job) -> Compiled {
    let canonical = canonical(job);
    let recovery = match &job.model {
        Model::Template {
            manifest,
            component,
        } if canonical.ir.is_none() => manifest
            .template(component)
            .ok()
            .map(|template| Arc::new(editor::recover(&job.text, template))),
        _ => None,
    };
    Compiled {
        canonical: Arc::new(canonical),
        recovery,
    }
}

fn canonical(job: &Job) -> CompileResult {
    match &job.model {
        Model::None => mesh_compiler::compile(&job.text),
        Model::Template {
            manifest,
            component,
        } => match manifest.template(component) {
            Ok(template) => {
                mesh_compiler::compile_with(&job.text, &CompileOptions::with_template(template))
            }
            // The server only asks for declared components; if one
            // isn't, the model-less compile is what D9 prescribes.
            Err(_) => mesh_compiler::compile(&job.text),
        },
    }
}

pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "a panic with no message".to_string()
    }
}
