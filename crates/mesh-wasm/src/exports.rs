//! The module's exports: the internal boundary the wrapper drives.
//!
//! One check is: `mesh_alloc` and fill a buffer per input; `mesh_check`
//! (or `mesh_compile`);
//! `mesh_free` every input buffer; read the document at
//! `mesh_result_ptr`/`mesh_result_len`; `mesh_result_clear`. After that,
//! the call holds no memory in the module.

use std::cell::RefCell;

/// The crate's version, which the wrapper checks before anything else.
const VERSION: &str = env!("CARGO_PKG_VERSION");

thread_local! {
    /// The last check's document, until `mesh_result_clear`. WebAssembly
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

/// The text of one input buffer, or `None` if it isn't UTF-8.
///
/// # Safety
///
/// `ptr` must point to `len` initialized bytes, unless `len` is 0.
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    // SAFETY: the caller guarantees `len` initialized bytes at `ptr`.
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }).ok()
}

/// Runs one check (`crate::respond`) and keeps its document for
/// [`mesh_result_ptr`]. Returns 0; or 1, keeping nothing, if an input
/// isn't UTF-8. With `has_model` 0, the three model inputs are ignored.
///
/// # Safety
///
/// Each pointer must point to its length's initialized bytes, unless that
/// length is 0.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn mesh_check(
    source: *const u8,
    source_len: usize,
    path: *const u8,
    path_len: usize,
    manifest: *const u8,
    manifest_len: usize,
    manifest_path: *const u8,
    manifest_path_len: usize,
    component: *const u8,
    component_len: usize,
    has_model: u32,
) -> u32 {
    // SAFETY: the caller's guarantee, passed on for each input.
    let inputs = unsafe {
        (
            text(source, source_len),
            text(path, path_len),
            text(manifest, manifest_len),
            text(manifest_path, manifest_path_len),
            text(component, component_len),
        )
    };
    let (Some(source), Some(path), Some(manifest), Some(manifest_path), Some(component)) = inputs
    else {
        mesh_result_clear();
        return 1;
    };
    let model = (has_model != 0).then_some((manifest, manifest_path, component));
    let document = crate::respond(source, path, model);
    RESULT.with(|result| *result.borrow_mut() = document.into_bytes());
    0
}

/// Runs one compile (`crate::respond_compile`) and keeps its result for
/// [`mesh_result_ptr`]: the same buffers as [`mesh_check`], always with a
/// model. Returns 0; or 1, keeping nothing, if an input isn't UTF-8.
///
/// # Safety
///
/// Each pointer must point to its length's initialized bytes, unless that
/// length is 0.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn mesh_compile(
    source: *const u8,
    source_len: usize,
    path: *const u8,
    path_len: usize,
    manifest: *const u8,
    manifest_len: usize,
    manifest_path: *const u8,
    manifest_path_len: usize,
    component: *const u8,
    component_len: usize,
) -> u32 {
    // SAFETY: the caller's guarantee, passed on for each input.
    let inputs = unsafe {
        (
            text(source, source_len),
            text(path, path_len),
            text(manifest, manifest_len),
            text(manifest_path, manifest_path_len),
            text(component, component_len),
        )
    };
    let (Some(source), Some(path), Some(manifest), Some(manifest_path), Some(component)) = inputs
    else {
        mesh_result_clear();
        return 1;
    };
    let result = crate::respond_compile(source, path, manifest, manifest_path, component);
    RESULT.with(|kept| *kept.borrow_mut() = result.into_bytes());
    0
}

/// Where the last check's document is.
#[no_mangle]
pub extern "C" fn mesh_result_ptr() -> *const u8 {
    RESULT.with(|result| result.borrow().as_ptr())
}

/// How many bytes the last check's document has.
#[no_mangle]
pub extern "C" fn mesh_result_len() -> usize {
    RESULT.with(|result| result.borrow().len())
}

/// Releases the last check's document.
#[no_mangle]
pub extern "C" fn mesh_result_clear() {
    RESULT.with(|result| *result.borrow_mut() = Vec::new());
}

/// Test-only exports: a panic, and a count of live allocations, Rust's
/// and Tree-sitter's C alike (its `malloc` is Rust's global allocator on
/// this target).
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
}
