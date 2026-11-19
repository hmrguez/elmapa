//! Drive fixed-rate, on-demand capture of the map camera's replay frames.
mod encode;

use crate::app::state::{Studio, ViewReplay};
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::gpu_readback::{Readback, ReadbackComplete},
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const FPS: u32 = 30;

#[derive(Resource, Default)]
pub struct VideoExport {
    dialog: Option<Task<Option<PathBuf>>>,
    recording: Option<Recording>,
    restart: bool,
}

impl VideoExport {
    pub fn busy(&self) -> bool {
        self.dialog.is_some() || self.recording.is_some()
    }

    pub(super) fn take_restart(&mut self) -> bool {
        std::mem::take(&mut self.restart)
    }

    pub(super) fn elapsed(&self, live_elapsed: f32) -> Option<f32> {
        match &self.recording {
            None => Some(live_elapsed),
            Some(recording) if matches!(recording.phase, Phase::Ready) => {
                Some(recording.index as f32 / FPS as f32)
            }
            Some(_) => None,
        }
    }

    pub(super) fn capture(&mut self, commands: &mut Commands, target: &RenderTarget, last: bool) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        let RenderTarget::Image(target) = target else {
            unreachable!("replay renders to the map texture");
        };
        let entity = commands
            .spawn(Readback::texture(target.handle.clone()))
            .observe(read_frame)
            .id();
        recording.phase = Phase::Reading {
            entity,
            last,
            requested: Instant::now(),
            armed: true,
        };
    }

    fn stop(&mut self, commands: &mut Commands) {
        self.dialog = None;
        self.restart = false;
        if let Some(recording) = self.recording.take()
            && let Phase::Reading { entity, .. } = recording.phase
        {
            commands.entity(entity).despawn();
        }
    }
}

struct Recording {
    source: UVec2,
    index: u64,
    phase: Phase,
    frames: Sender<Frame>,
    updates: Receiver<EncoderUpdate>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

enum Phase {
    Ready,
    Reading {
        entity: Entity,
        last: bool,
        requested: Instant,
        armed: bool,
    },
    Encoding,
    Finishing,
}

struct Frame {
    pixels: Vec<u8>,
    last: bool,
}

enum EncoderUpdate {
    Encoded,
    Finished(Result<(), String>),
}

impl Recording {
    fn start(destination: PathBuf, replace: bool, source: UVec2) -> std::io::Result<Self> {
        if source.min_element() < 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the map view is too small to record",
            ));
        }
        let (frames, input) = bounded(1);
        let (output, updates) = bounded(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        std::thread::Builder::new()
            .name("elmapa-video".into())
            .spawn(move || {
                let result = encode::run(&destination, replace, source, input, &output, &stop);
                let _ = output.send(EncoderUpdate::Finished(result));
            })?;
        Ok(Self {
            source,
            index: 0,
            phase: Phase::Ready,
            frames,
            updates,
            cancelled,
        })
    }

    fn poll(&mut self, commands: &mut Commands, size: UVec2) -> Option<Result<(), String>> {
        // A resize after the last capture can leave the completed video intact.
        if size != self.source && !matches!(self.phase, Phase::Finishing) {
            return Some(Err(
                "the window was resized. Save again to use the new view.".into(),
            ));
        }
        if let Phase::Reading {
            entity,
            requested,
            armed,
            ..
        } = &mut self.phase
        {
            if std::mem::take(armed) {
                // Readback extracts the component every frame. Schedule a single copy.
                commands.entity(*entity).remove::<Readback>();
            }
            if requested.elapsed() > Duration::from_secs(10) {
                return Some(Err("the map renderer did not return a frame.".into()));
            }
        }
        match self.updates.try_recv() {
            Ok(EncoderUpdate::Encoded) => {
                self.index += 1;
                self.phase = Phase::Ready;
                None
            }
            Ok(EncoderUpdate::Finished(result)) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the video encoder stopped.".into())),
        }
    }
}

pub fn prepare(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    mut video: ResMut<VideoExport>,
    window: Single<&Window>,
) {
    if studio.replay_view.is_none() {
        if video.busy() {
            video.stop(&mut commands);
        }
        return;
    }
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let requested = std::mem::take(&mut studio.replay_video_requested);
    if requested && !video.busy() {
        studio.status.clear();
        studio.ui_dirty = true;
        video.dialog = Some(IoTaskPool::get().spawn(async {
            rfd::AsyncFileDialog::new()
                .add_filter("MP4 video", &["mp4"])
                .set_file_name("Replay.mp4")
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
        }));
    }
    if let Some(dialog) = &mut video.dialog
        && let Some(destination) = block_on(poll_once(dialog))
    {
        video.dialog = None;
        studio.ui_dirty = true;
        if let Some(mut destination) = destination {
            // Only the exact path returned by the save panel can be replaced.
            let replace = destination.extension().is_some();
            if !replace {
                destination.set_extension("mp4");
            }
            if !destination
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
            {
                studio.status = "Could not save replay video. Use an .mp4 filename.".into();
            } else {
                match Recording::start(destination, replace, size) {
                    Ok(recording) => {
                        video.recording = Some(recording);
                        video.restart = true;
                        studio.replay_view = Some(ViewReplay::Playing);
                    }
                    Err(error) => studio.status = format!("Could not save replay video: {error}"),
                }
            }
        }
    }
    if let Some(recording) = &mut video.recording
        && let Some(result) = recording.poll(&mut commands, size)
    {
        video.stop(&mut commands);
        studio.status = match result {
            Ok(()) => "Replay video saved.".into(),
            Err(error) => {
                // End a partial export on the complete map.
                if studio.replay_view == Some(ViewReplay::Playing) {
                    studio.replay_view = Some(ViewReplay::Finished);
                }
                format!("Could not save replay video: {error}")
            }
        };
        studio.ui_dirty = true;
    }
}

fn read_frame(
    mut event: On<ReadbackComplete>,
    mut commands: Commands,
    mut video: ResMut<VideoExport>,
) {
    let Some(recording) = &mut video.recording else {
        return;
    };
    let Phase::Reading { entity, last, .. } = recording.phase else {
        return;
    };
    if event.entity != entity {
        return;
    }
    commands.entity(entity).despawn();
    let _ = recording.frames.try_send(Frame {
        pixels: std::mem::take(&mut event.event_mut().data),
        last,
    });
    recording.phase = if last {
        Phase::Finishing
    } else {
        Phase::Encoding
    };
}
