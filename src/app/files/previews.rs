//! Generate and retain shelf previews independently of document I/O.
use crate::app::{render, state::Studio, ui};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use elmapa::scene;
use std::collections::{HashMap, HashSet};

type PreviewResult = (u64, u64, Result<Vec<u8>, String>);

#[derive(Default)]
pub(super) struct ShelfPreviews {
    images: HashMap<u64, Handle<Image>>,
    task: Option<Task<PreviewResult>>,
    failed: HashSet<u64>,
    epoch: u64,
}

impl ShelfPreviews {
    pub(super) fn get(&self, id: u64) -> Option<&Handle<Image>> {
        self.images.get(&id)
    }

    pub(super) fn invalidate(&mut self) {
        self.epoch += 1;
        self.images.clear();
        self.failed.clear();
    }

    pub(super) fn poll(
        &mut self,
        studio: &Studio,
        switching: bool,
        images: &mut Assets<Image>,
        slots: Query<(&ui::PreviewSlot, &ComputedNode, &UiGlobalTransform)>,
        window: &Window,
    ) -> bool {
        let mut published = false;
        if let Some(mut task) = self.task.take() {
            if let Some((id, epoch, result)) = block_on(poll_once(&mut task)) {
                if epoch == self.epoch {
                    match result {
                        Ok(mut bytes) => {
                            for p in bytes.chunks_exact_mut(4) {
                                p.swap(0, 2);
                            }
                            if self.images.len() >= 16
                                && let Some(old) =
                                    self.images.keys().copied().find(|old| *old != id)
                            {
                                self.images.remove(&old);
                            }
                            self.images.insert(
                                id,
                                images.add(render::rgba_image(
                                    scene::thumbnail::WIDTH,
                                    scene::thumbnail::HEIGHT,
                                    bytes,
                                )),
                            );
                            published = true;
                        }
                        Err(error) => {
                            self.failed.insert(id);
                            warn!("Atlas preview {id}: {error}");
                        }
                    }
                }
            } else {
                self.task = Some(task);
            }
        }
        if studio.home
            && !switching
            && self.task.is_none()
            && let Some(library) = &studio.library
        {
            let visible: Vec<_> = slots
                .iter()
                .filter(|(_, node, transform)| {
                    let y = transform.translation.y * node.inverse_scale_factor();
                    y + node.size().y * node.inverse_scale_factor() / 2. > 100.
                        && y - node.size().y * node.inverse_scale_factor() / 2. < window.height()
                })
                .map(|(slot, _, _)| slot.0)
                .collect();
            if let Some(record) = library
                .shelf
                .atlases
                .iter()
                .find(|r| {
                    visible.contains(&r.id)
                        && !self.images.contains_key(&r.id)
                        && !self.failed.contains(&r.id)
                })
                .cloned()
            {
                let library = library.clone();
                let id = record.id;
                let epoch = self.epoch;
                self.task = Some(
                    IoTaskPool::get().spawn(async move { (id, epoch, library.preview(&record)) }),
                );
            }
        }
        published
    }
}
