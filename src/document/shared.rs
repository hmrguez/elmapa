//! COW collections keep editable JSON unchanged while revisions share payloads.
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SharedVec<T>(
    Arc<Vec<T>>,
    #[serde(skip)] Arc<std::sync::OnceLock<Option<super::ContentBounds>>>,
);
impl<T> Default for SharedVec<T> {
    fn default() -> Self {
        Self(Arc::new(Vec::new()), Arc::default())
    }
}
impl<T: PartialEq> PartialEq for SharedVec<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}
impl<T> SharedVec<T> {
    pub fn storage_id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Deref for SharedVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Clone> DerefMut for SharedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.1 = Arc::default();
        Arc::make_mut(&mut self.0)
    }
}
impl<T> From<Vec<T>> for SharedVec<T> {
    fn from(value: Vec<T>) -> Self {
        Self(Arc::new(value), Arc::default())
    }
}
impl<T> FromIterator<T> for SharedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Vec::from_iter(iter).into()
    }
}
impl<'a, T> IntoIterator for &'a SharedVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut SharedVec<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T: Clone> IntoIterator for SharedVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.0).into_iter()
    }
}

impl SharedVec<super::Point> {
    pub fn point_bounds(&self) -> Option<super::ContentBounds> {
        *self.1.get_or_init(|| {
            let first = *self.first()?;
            Some(self.iter().copied().fold(
                super::ContentBounds {
                    min: first,
                    max: first,
                },
                |bounds, q| bounds.union(super::ContentBounds { min: q, max: q }),
            ))
        })
    }
    pub fn push_point(&mut self, point: super::Point) {
        let bounds = self.point_bounds().map_or(
            super::ContentBounds {
                min: point,
                max: point,
            },
            |bounds| {
                bounds.union(super::ContentBounds {
                    min: point,
                    max: point,
                })
            },
        );
        Arc::make_mut(&mut self.0).push(point);
        self.1 = Arc::default();
        let _ = self.1.set(Some(bounds));
    }
}
