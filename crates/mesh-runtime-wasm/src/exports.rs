//! The module's exports: the internal boundary the wrapper drives.
//!
//! One call is: `mesh_alloc` and fill a buffer per input; `mesh_render`
//! or `mesh_dispatch`; `mesh_free` every input buffer; read the result at
//! `mesh_result_ptr`/`mesh_result_len`; `mesh_result_clear`. After that,
//! the call holds no memory in the module.
//!
//! A call returns a status: 0, and the result document; 1, and nothing,
//! if an input isn't UTF-8 or isn't the encoding (the wrapper's fault);
//! or 2, and a message, if the host's value can't be taken at all (a
//! snapshot that isn't a record, or a value nested too deeply).

use crate::Refusal;
use std::cell::RefCell;

/// The crate's version, which the wrapper checks before anything else.
const VERSION: &str = env!("CARGO_PKG_VERSION");

thread_local! {
    /// The last call's result, until `mesh_result_clear`. WebAssembly
    /// here is single-threaded; `thread_local!` keeps `static mut` out.
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Where the version's UTF-8 bytes are. Never changes (see the crate docs).
#[no_mangle]
pub extern "C" fn mesh_version_ptr() -> *const u8 {
    VERSION.as_ptr()
}

/// How many bytes the version has. Never changes (see the crate docs).
#[no_mangle]
pub extern "C" fn mesh_version_len() -> usize {
    VERSION.len()
}

/// A buffer of `len` bytes for the wrapper to fill, released with
/// [`mesh_free`] and the same `len`.
#[no_mangle]
pub extern "C" fn mesh_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Releases a buffer from [`mesh_alloc`].
///
/// # Safety
///
/// `ptr` and `len` must be exactly what one call to `mesh_alloc` gave and
/// was given, and the buffer must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn mesh_free(ptr: *mut u8, len: usize) {
    // SAFETY: the caller passes back one allocation of `len` bytes' capacity.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

/// One input buffer's bytes.
///
/// # Safety
///
/// `ptr` must point to `len` initialized bytes, unless `len` is 0.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the caller guarantees `len` initialized bytes at `ptr`.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// One input buffer's text, or `None` if it isn't UTF-8.
///
/// # Safety
///
/// As for [`bytes`].
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    // SAFETY: the caller's guarantee, passed on.
    std::str::from_utf8(unsafe { bytes(ptr, len) }).ok()
}

/// Keeps `result` for [`mesh_result_ptr`], and gives its status.
fn keep(result: Option<Result<String, Refusal>>) -> u32 {
    let (status, kept) = match result {
        Some(Ok(document)) => (0, document),
        Some(Err(Refusal::Input(message))) => (2, message),
        None | Some(Err(Refusal::Encoding(_))) => (1, String::new()),
    };
    RESULT.with(|result| *result.borrow_mut() = kept.into_bytes());
    status
}

/// Renders (`crate::respond_render`): the root's name, the templates as a
/// text list, the manifest, and the snapshot, encoded.
///
/// # Safety
///
/// Each pointer must point to its length's initialized bytes, unless that
/// length is 0.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn mesh_render(
    root: *const u8,
    root_len: usize,
    templates: *const u8,
    templates_len: usize,
    model: *const u8,
    model_len: usize,
    snapshot: *const u8,
    snapshot_len: usize,
) -> u32 {
    // SAFETY: the caller's guarantee, passed on for each input.
    let result = unsafe {
        match (text(root, root_len), text(model, model_len)) {
            (Some(root), Some(model)) => Some(crate::respond_render(
                root,
                bytes(templates, templates_len),
                model,
                bytes(snapshot, snapshot_len),
            )),
            _ => None,
        }
    };
    keep(result)
}

/// Dispatches (`crate::respond_dispatch`): a render's inputs, as
/// [`mesh_render`] took them, then the handler identifier, and the
/// payload, encoded, which is absent when `has_payload` is 0.
///
/// # Safety
///
/// Each pointer must point to its length's initialized bytes, unless that
/// length is 0.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn mesh_dispatch(
    root: *const u8,
    root_len: usize,
    templates: *const u8,
    templates_len: usize,
    model: *const u8,
    model_len: usize,
    snapshot: *const u8,
    snapshot_len: usize,
    handler: *const u8,
    handler_len: usize,
    payload: *const u8,
    payload_len: usize,
    has_payload: u32,
) -> u32 {
    // SAFETY: the caller's guarantee, passed on for each input.
    let result = unsafe {
        match (
            text(root, root_len),
            text(model, model_len),
            text(handler, handler_len),
        ) {
            (Some(root), Some(model), Some(handler)) => Some(crate::respond_dispatch(
                root,
                bytes(templates, templates_len),
                model,
                bytes(snapshot, snapshot_len),
                handler,
                (has_payload != 0).then(|| bytes(payload, payload_len)),
            )),
            _ => None,
        }
    };
    keep(result)
}

/// Where the last call's result is.
#[no_mangle]
pub extern "C" fn mesh_result_ptr() -> *const u8 {
    RESULT.with(|result| result.borrow().as_ptr())
}

/// How many bytes the last call's result has.
#[no_mangle]
pub extern "C" fn mesh_result_len() -> usize {
    RESULT.with(|result| result.borrow().len())
}

/// Releases the last call's result.
#[no_mangle]
pub extern "C" fn mesh_result_clear() {
    RESULT.with(|result| *result.borrow_mut() = Vec::new());
}

/// Test-only exports: a panic, a count of live allocations, and number
/// to text in batches.
#[cfg(feature = "test-hooks")]
mod hooks {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static LIVE: AtomicUsize = AtomicUsize::new(0);

    struct Counting;

    // SAFETY: every method forwards to `System` unchanged, and only counts.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: forwarded with the caller's guarantees.
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                LIVE.fetch_add(1, Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: forwarded with the caller's guarantees.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                LIVE.fetch_add(1, Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: forwarded with the caller's guarantees.
            unsafe { System.dealloc(ptr, layout) };
            LIVE.fetch_sub(1, Ordering::Relaxed);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: forwarded with the caller's guarantees. A reallocation
            // keeps one allocation live, moved or not.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// How many allocations are live.
    #[no_mangle]
    pub extern "C" fn mesh_live_allocations() -> usize {
        LIVE.load(Ordering::Relaxed)
    }

    /// Panics, which traps: the failure tests' way to cause one.
    #[no_mangle]
    pub extern "C" fn mesh_test_panic() {
        panic!("a panic on purpose, from a test-only export");
    }

    /// MPRX's text for each number in the buffer (`crate::number_texts`),
    /// kept for `mesh_result_ptr`. Returns 0, or 1 keeping nothing.
    ///
    /// # Safety
    ///
    /// `bits` must point to `len` initialized bytes, unless `len` is 0.
    #[no_mangle]
    pub unsafe extern "C" fn mesh_test_number_text(bits: *const u8, len: usize) -> u32 {
        // SAFETY: the caller's guarantee, passed on.
        let texts = crate::number_texts(unsafe { super::bytes(bits, len) });
        let status = u32::from(texts.is_none());
        super::RESULT.with(|result| *result.borrow_mut() = texts.unwrap_or_default().into_bytes());
        status
    }
}
