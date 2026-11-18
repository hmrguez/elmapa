//! Single bounded I/O worker. Switch/quit waits for a durable local checkpoint.
mod previews;

use super::{state::Studio, ui};
use bevy::{
    app::AppExit,
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use elmapa::{
    document::Document,
    library::{AtlasRecord, Camera, Library},
    storage::{DocumentStore, LocalDocumentStore, export_svg},
};
use previews::ShelfPreviews;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone)]
pub enum Request {
    Home,
    Enter(u64),
    Create(bool),
    Save(Option<PathBuf>),
    Open(Option<PathBuf>),
    Export(Option<PathBuf>),
    Quit,
    Preferences,
}
struct Completion {
    library: Option<Library>,
    result: Result<Outcome, String>,
    epoch: u64,
}
enum Outcome {
    Loaded,
    Home,
    Enter(Document, AtlasRecord, bool),
    Saved(PathBuf),
    Exported,
    Quit,
    Preferences,
    Cancelled,
}
#[derive(Resource, Default)]
pub struct Jobs {
    task: Option<Task<Completion>>,
    pending: Option<Request>,
    pub preferences: bool,
    switching: bool,
    started: bool,
    previews: ShelfPreviews,
    export_token: Arc<AtomicU64>,
}
impl Jobs {
    pub fn request(&mut self, request: Request) {
        self.pending = Some(request);
    }
    pub fn preview(&self, id: u64) -> Option<&Handle<Image>> {
        self.previews.get(id)
    }
    pub fn cancel_export(&self) {
        self.export_token.fetch_add(1, Ordering::Relaxed);
    }
    pub fn switching(&self) -> bool {
        self.switching
    }
    pub fn busy(&self) -> bool {
        self.task.is_some() || self.pending.is_some()
    }
}

pub fn poll(
    mut studio: ResMut<Studio>,
    mut jobs: ResMut<Jobs>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut images: ResMut<Assets<Image>>,
    slots: Query<(&ui::PreviewSlot, &ComputedNode, &UiGlobalTransform)>,
    window: Single<&Window>,
) {
    if !jobs.started {
        jobs.started = true;
        jobs.task = Some(IoTaskPool::get().spawn(async {
            let result = (|| {
                let mut library = Library::load(Library::default_path())?;
                if library.shelf.atlases.is_empty() {
                    let doc = Document::demo();
                    library.checkpoint(
                        AtlasRecord {
                            id: 1,
                            name: doc.name.clone(),
                            source: None,
                            camera: Camera::for_document(&doc),
                        },
                        &doc,
                    )?;
                }
                Ok::<_, String>(library)
            })();
            match result {
                Ok(library) => Completion {
                    library: Some(library),
                    result: Ok(Outcome::Loaded),
                    epoch: 0,
                },
                Err(error) => Completion {
                    library: None,
                    result: Err(error),
                    epoch: 0,
                },
            }
        }));
    }
    if let Some(mut task) = jobs.task.take() {
        if let Some(completion) = block_on(poll_once(&mut task)) {
            jobs.switching = false;
            if let Some(library) = completion.library {
                studio.library = Some(library);
            }
            match completion.result {
                Ok(Outcome::Loaded) => {
                    if let Some(library) = &studio.library {
                        let dark = library.shelf.dark || studio.dark;
                        let reduced = library.shelf.reduced_motion;
                        studio.disabled_landmark_packs =
                            library.shelf.disabled_landmark_packs.clone();
                        studio.dark = dark;
                        studio.reduced = reduced;
                    }
                    studio.status = "Choose a world to explore.".into();
                }
                Ok(Outcome::Home) => {
                    studio.home = true;
                    studio.settings = std::mem::take(&mut studio.settings_on_home);
                    studio.dirty = false;
                    jobs.previews.invalidate();
                }
                Ok(Outcome::Enter(document, record, book)) => {
                    studio.opening_image = jobs.preview(record.id).cloned();
                    studio.status = "Welcome back to your world.".into();
                    studio.install_document(
                        document,
                        record.id,
                        record.source,
                        Some(record.camera),
                        time.elapsed_secs(),
                    );
                    if !book {
                        studio.opening = None;
                        studio.status = "Opening atlas…".into();
                    }
                }
                Ok(Outcome::Saved(path)) => {
                    studio.path = Some(path);
                    if completion.epoch == studio.scene.epoch() {
                        studio.dirty = false;
                        studio.status = "Saved your world.".into();
                    } else {
                        studio.status =
                            "Saved the requested snapshot; newer changes remain unsaved.".into();
                    }
                }
                Ok(Outcome::Exported) => studio.status = "Exported SVG.".into(),
                Ok(Outcome::Quit) => {
                    exit.write(AppExit::Success);
                }
                Ok(Outcome::Preferences) => {}
                Ok(Outcome::Cancelled) => studio.status = "File operation cancelled.".into(),
                Err(error) => {
                    studio.status = format!("Could not complete file operation: {error}");
                    studio.quit_requested = false;
                }
            }
            studio.ui_dirty = true;
        } else {
            jobs.task = Some(task);
        }
    }
    if jobs.task.is_none() {
        let request = jobs
            .pending
            .take()
            .or_else(|| jobs.preferences.then_some(Request::Preferences));
        if let Some(request) = request {
            let Some(mut library) = studio.library.clone() else {
                studio.status = "Could not open the library; existing files have been kept.".into();
                studio.ui_dirty = true;
                if matches!(request, Request::Quit) {
                    exit.write(AppExit::Success);
                }
                return;
            };
            library.shelf.dark = studio.dark;
            library.shelf.reduced_motion = studio.reduced;
            library.shelf.disabled_landmark_packs = studio.disabled_landmark_packs.clone();
            let external = matches!(request, Request::Save(_) | Request::Export(_));
            if !external {
                jobs.preferences = false;
            }
            jobs.switching = matches!(
                request,
                Request::Home
                    | Request::Enter(_)
                    | Request::Create(_)
                    | Request::Open(_)
                    | Request::Quit
            );
            let document = studio.editor.document().clone();
            let epoch = studio.scene.epoch();
            let record = studio.atlas.map(|id| AtlasRecord {
                id,
                name: document.name.clone(),
                source: studio.path.clone(),
                camera: studio.viewport.camera(studio.bounds),
            });
            let path = studio.path.clone();
            let home = studio.home;
            let token = jobs.export_token.clone();
            let version = token.load(Ordering::Relaxed);
            studio.status = match request {
                Request::Save(_) => "Saving…",
                Request::Export(_) => "Exporting…",
                Request::Open(_) | Request::Enter(_) => "Opening…",
                _ => "Preparing…",
            }
            .into();
            studio.ui_dirty = true;
            jobs.task = Some(IoTaskPool::get().spawn(async move {
                let result = (|| -> Result<Outcome, String> {
                    if !external {
                        if !home
                            && matches!(
                                request,
                                Request::Home
                                    | Request::Enter(_)
                                    | Request::Create(_)
                                    | Request::Open(_)
                                    | Request::Quit
                            )
                            && let Some(record) = record
                        {
                            library.checkpoint(record, &document)?;
                        }
                        library.save_preferences()?;
                    }
                    match request {
                        Request::Home => Ok(Outcome::Home),
                        Request::Quit => Ok(Outcome::Quit),
                        Request::Preferences => Ok(Outcome::Preferences),
                        Request::Enter(id) => {
                            let record = library
                                .shelf
                                .atlases
                                .iter()
                                .find(|r| r.id == id)
                                .cloned()
                                .ok_or("Atlas no longer exists")?;
                            Ok(Outcome::Enter(library.read(id)?, record, true))
                        }
                        Request::Create(demo) => {
                            let id = library.next_id();
                            let mut document = if demo {
                                Document::demo()
                            } else {
                                Document::blank()
                            };
                            if !demo {
                                document.name = format!("Untitled atlas {id:02}");
                            }
                            let record = AtlasRecord {
                                id,
                                name: document.name.clone(),
                                source: None,
                                camera: Camera::for_document(&document),
                            };
                            library.checkpoint(record.clone(), &document)?;
                            Ok(Outcome::Enter(document, record, true))
                        }
                        Request::Save(destination) => {
                            let destination = destination.or(path).or_else(|| {
                                rfd::FileDialog::new()
                                    .add_filter("Elmapa atlas", &["elmapa"])
                                    .set_file_name("world.elmapa")
                                    .save_file()
                            });
                            let Some(destination) = destination else {
                                return Ok(Outcome::Cancelled);
                            };
                            LocalDocumentStore.save(&document, &destination)?;
                            Ok(Outcome::Saved(destination))
                        }
                        Request::Export(destination) => {
                            let destination = destination.or_else(|| {
                                rfd::FileDialog::new()
                                    .add_filter("SVG image", &["svg"])
                                    .set_file_name("world.svg")
                                    .save_file()
                            });
                            let Some(destination) = destination else {
                                return Ok(Outcome::Cancelled);
                            };
                            export_svg(&destination, &document, Some((token, version)))?;
                            Ok(Outcome::Exported)
                        }
                        Request::Open(source) => {
                            let source = source.or_else(|| {
                                rfd::FileDialog::new()
                                    .add_filter("Elmapa atlas", &["elmapa"])
                                    .pick_file()
                            });
                            let Some(source) = source else {
                                return Ok(Outcome::Cancelled);
                            };
                            let document = LocalDocumentStore.load(&source)?;
                            let id = library
                                .shelf
                                .atlases
                                .iter()
                                .find(|a| a.source.as_ref() == Some(&source))
                                .map_or_else(|| library.next_id(), |a| a.id);
                            let record = AtlasRecord {
                                id,
                                name: document.name.clone(),
                                source: Some(source),
                                camera: Camera::for_document(&document),
                            };
                            library.checkpoint(record.clone(), &document)?;
                            Ok(Outcome::Enter(document, record, false))
                        }
                    }
                })();
                Completion {
                    library: Some(library),
                    result,
                    epoch,
                }
            }));
        }
    }
    let switching = jobs.switching();
    if jobs
        .previews
        .poll(&studio, switching, &mut images, slots, &window)
    {
        studio.ui_dirty = true;
    }
}

#[cfg(test)]
mod tests;
