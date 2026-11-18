//! Optional allocation activity counts; layout/ownership always stay with System.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);
pub struct CountingAllocator;
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}
pub fn snapshot() -> (u64, u64, u64) {
    (
        CALLS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
        FREES.load(Ordering::Relaxed),
    )
}
fn allocation(bytes: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}
// SAFETY: every operation forwards the original pointer, layout, and size to
// System. The counters do not allocate, alter alignment, or retain allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocation(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            allocation(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ENABLED.load(Ordering::Relaxed) {
            FREES.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, size) };
        if !new.is_null() {
            allocation(size);
        }
        new
    }
}
