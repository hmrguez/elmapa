//! Cooperative generation limits. A failed/cancelled result is never publishable.
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Copy)]
pub struct Limits {
    pub vertices: usize,
    pub samples: usize,
    pub cells: usize,
}
impl Limits {
    pub const INTERACTIVE: Self = Self {
        vertices: 4_000_000,
        samples: 1_000_000,
        // Fixed-resolution terrain samples cover long coasts without rescaling them.
        cells: 4_000_000,
    };
    pub const EXPORT: Self = Self {
        vertices: 16_000_000,
        samples: 4_000_000,
        cells: 16_000_000,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationError {
    Cancelled,
    Budget,
}
impl std::error::Error for GenerationError {}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Generation superseded by newer input.",
            Self::Budget => "This view exceeds the generated geometry budget. Artwork is preserved; undo an edit or simplify the document.",
        })
    }
}
struct State {
    token: Option<(Arc<AtomicU64>, u64)>,
    limits: Limits,
    vertices: usize,
    samples: usize,
    cells: usize,
    error: Option<GenerationError>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn bounded<T>(
    limits: Limits,
    token: Option<(Arc<AtomicU64>, u64)>,
    f: impl FnOnce() -> T,
) -> Result<T, GenerationError> {
    struct Guard(Option<State>);
    impl Drop for Guard {
        fn drop(&mut self) {
            STATE.with(|state| *state.borrow_mut() = self.0.take());
        }
    }
    let guard = Guard(STATE.with(|state| {
        state.replace(Some(State {
            token,
            limits,
            vertices: 0,
            samples: 0,
            cells: 0,
            error: None,
        }))
    }));
    let output = f();
    let error = STATE.with(|state| state.borrow().as_ref().and_then(|s| s.error));
    drop(guard);
    error.map_or(Ok(output), Err)
}

pub(super) fn stopped() -> bool {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return false;
        };
        if state
            .token
            .as_ref()
            .is_some_and(|(token, revision)| token.load(Ordering::Relaxed) != *revision)
        {
            state.error = Some(GenerationError::Cancelled);
        }
        state.error.is_some()
    })
}
fn charge(vertices: usize, samples: usize, cells: usize) -> bool {
    if stopped() {
        return false;
    }
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return true;
        };
        state.vertices = state.vertices.saturating_add(vertices);
        state.samples = state.samples.saturating_add(samples);
        state.cells = state.cells.saturating_add(cells);
        if state.vertices > state.limits.vertices
            || state.samples > state.limits.samples
            || state.cells > state.limits.cells
        {
            state.error = Some(GenerationError::Budget);
            return false;
        }
        true
    })
}
pub(super) fn vertices(n: usize) -> bool {
    charge(n, 0, 0)
}
pub(super) fn samples(n: usize) -> bool {
    charge(0, n, 0)
}
pub(super) fn cells(n: usize) -> bool {
    charge(0, 0, n)
}

/// Native adapters share the job's generated-work budget before tessellation.
pub fn vertices_for_native(n: usize) -> bool {
    vertices(n)
}
/// Stop adapter preparation after superseded input or a generated-work limit.
pub fn is_stopped() -> bool {
    stopped()
}
