//! Deterministic work counters, separate from machine-specific timing gates.
use std::cell::Cell;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkCounts {
    pub field_samples: usize,
    pub contour_cells: usize,
    pub forest_dabs: usize,
    pub resampled_segments: usize,
    pub forest_candidates: usize,
    pub line_strokes: usize,
    pub region_batches: usize,
}
thread_local! { static WORK: Cell<WorkCounts> = const { Cell::new(WorkCounts { field_samples: 0, contour_cells: 0, forest_dabs: 0, resampled_segments: 0, forest_candidates: 0, line_strokes: 0, region_batches: 0 }) }; }
pub(super) fn update(f: impl FnOnce(&mut WorkCounts)) {
    WORK.with(|work| {
        let mut counts = work.get();
        f(&mut counts);
        work.set(counts);
    });
}
pub(super) fn take() -> WorkCounts {
    WORK.with(|work| work.replace(WorkCounts::default()))
}
