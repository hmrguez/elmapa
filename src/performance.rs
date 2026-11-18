//! Opt-in bounded native traces. Disabled runs neither time nor log hot events.
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
const EVENTS: usize = 131_072;
#[derive(Serialize)]
struct Event {
    name: String,
    at_us: u64,
    duration_us: u64,
    value: u64,
    display_time: Option<f64>,
}
#[derive(Default, Serialize)]
struct Trace {
    events: VecDeque<Event>,
    dropped: u64,
    #[serde(skip)]
    inputs: HashMap<u64, u64>,
}
fn epoch() -> &'static Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now)
}
fn now() -> u64 {
    epoch().elapsed().as_micros() as u64
}
fn trace() -> &'static Mutex<Trace> {
    static TRACE: OnceLock<Mutex<Trace>> = OnceLock::new();
    TRACE.get_or_init(|| Mutex::new(Trace::default()))
}
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("ELMAPA_TRACE").is_some())
}
fn push(event: Event) {
    let mut trace = trace().lock().unwrap_or_else(|e| e.into_inner());
    if trace.events.len() == EVENTS {
        trace.events.pop_front();
        trace.dropped += 1;
    }
    trace.events.push_back(event);
}
pub struct Span(Option<(&'static str, Instant)>);
impl Span {
    pub fn new(name: &'static str) -> Self {
        Self(enabled().then(|| {
            let _ = epoch();
            (name, Instant::now())
        }))
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((name, start)) = self.0 {
            push(Event {
                name: name.into(),
                at_us: now(),
                duration_us: start.elapsed().as_micros() as u64,
                value: 0,
                display_time: None,
            });
        }
    }
}
/// Record an explicitly measured duration without conflating it with presentation.
pub fn duration(name: &'static str, duration: std::time::Duration) {
    if enabled() {
        push(Event {
            name: name.into(),
            at_us: now(),
            duration_us: duration.as_micros() as u64,
            value: 0,
            display_time: None,
        });
    }
}
pub fn counter(name: &'static str, value: u64) {
    if enabled() {
        push(Event {
            name: name.into(),
            at_us: now(),
            duration_us: 0,
            value,
            display_time: None,
        });
    }
}
static INPUT: AtomicU64 = AtomicU64::new(0);
pub fn input() {
    if enabled() {
        let time = now().max(1);
        INPUT.store(time, Ordering::Relaxed);
        counter("input", time);
    }
}

#[derive(Clone, Copy)]
pub enum FrameStage {
    Started,
    CpuSubmitted,
    GpuCompleted {
        seconds: f64,
    },
    Resources {
        device_bytes: u64,
        atlas_bytes: u64,
        path_bytes: u64,
        sprite_instances: u64,
    },
    Presented {
        seconds: f64,
    },
}
pub struct FrameEvent {
    pub id: u64,
    pub started: Instant,
    pub stage: FrameStage,
}
static PRESENTED_INPUT: AtomicU64 = AtomicU64::new(0);

/// Metal drawable presentation timestamps support real presented-frame intervals.
/// Callback latency is reported separately; it is not input-to-photon latency.
pub fn native_frame(event: FrameEvent) {
    let at_us = now();
    let mut display_time = None;
    let (name, duration_us, value) = match event.stage {
        FrameStage::Started => {
            let mut trace = trace().lock().unwrap_or_else(|e| e.into_inner());
            if trace.inputs.len() >= 512 {
                trace.inputs.clear();
            }
            trace.inputs.insert(event.id, INPUT.load(Ordering::Relaxed));
            ("frame_begin", 0, event.id)
        }
        FrameStage::CpuSubmitted => (
            "metal_submit",
            event.started.elapsed().as_micros() as u64,
            event.id,
        ),
        FrameStage::Resources {
            device_bytes,
            atlas_bytes,
            path_bytes,
            sprite_instances,
        } => {
            counter("native_color_sprite_instances", sprite_instances);
            counter("gpu_device_allocated_bytes", device_bytes);
            counter("gpu_atlas_allocated_bytes", atlas_bytes);
            counter("gpu_path_target_bytes", path_bytes);
            memory_counters();
            return;
        }
        FrameStage::GpuCompleted { seconds } => (
            "gpu_execution",
            (seconds.max(0.) * 1_000_000.) as u64,
            event.id,
        ),
        FrameStage::Presented { seconds } => {
            display_time = Some(seconds);
            let input = trace()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .inputs
                .remove(&event.id)
                .unwrap_or(0);
            if input > 0 && PRESENTED_INPUT.fetch_max(input, Ordering::Relaxed) < input {
                push(Event {
                    name: "input_to_present_callback".into(),
                    at_us,
                    duration_us: at_us.saturating_sub(input),
                    value: event.id,
                    display_time,
                });
            }
            (
                "presented",
                event.started.elapsed().as_micros() as u64,
                event.id,
            )
        }
    };
    push(Event {
        name: name.into(),
        at_us,
        duration_us,
        value,
        display_time,
    });
}
pub fn flush() -> Result<(), String> {
    let Some(path) = std::env::var_os("ELMAPA_TRACE") else {
        return Ok(());
    };
    memory_counters();
    let trace = trace().lock().unwrap_or_else(|e| e.into_inner());
    let bytes = serde_json::to_vec(&*trace).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn mark(label: &str) {
    if enabled() {
        push(Event {
            name: format!("mark:{label}"),
            at_us: now(),
            duration_us: 0,
            value: 0,
            display_time: None,
        });
    }
}

#[cfg(target_os = "macos")]
fn resident_bytes() -> Option<(u64, u64)> {
    // task_info writes exactly MACH_TASK_BASIC_INFO_COUNT integer words into the
    // matching C struct; both the task port and pointer live for this call.
    unsafe {
        let mut info: libc::mach_task_basic_info = std::mem::zeroed();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        let status = libc::task_info(
            mach2::traps::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut libc::integer_t,
            &mut count,
        );
        (status == libc::KERN_SUCCESS).then_some((info.resident_size, info.resident_size_max))
    }
}
#[cfg(not(target_os = "macos"))]
fn resident_bytes() -> Option<(u64, u64)> {
    None
}

fn memory_counters() {
    if let Some((resident, peak)) = resident_bytes() {
        counter("cpu_resident_bytes", resident);
        counter("cpu_peak_resident_bytes", peak);
    }
    let (calls, bytes, frees) = crate::allocation::snapshot();
    counter("rust_allocation_calls", calls);
    counter("rust_allocated_bytes_gross", bytes);
    counter("rust_deallocation_calls", frees);
}
