//! Persistent stroke chunks: appending an edit copies at most 64 stroke records.
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};
use std::{
    ops::{Index, IndexMut},
    sync::Arc,
};
const CHUNK: usize = 64;
#[derive(Debug, Clone)]
pub struct SharedChunks<T> {
    chunks: Arc<Vec<Arc<Vec<T>>>>,
    len: usize,
    brush_summary: std::sync::OnceLock<u8>,
}
impl<T> Default for SharedChunks<T> {
    fn default() -> Self {
        Self {
            chunks: Arc::default(),
            len: 0,
            brush_summary: std::sync::OnceLock::new(),
        }
    }
}
fn elements<T>(chunk: &Arc<Vec<T>>) -> std::slice::Iter<'_, T> {
    chunk.iter()
}
pub type ChunkIter<'a, T> = std::iter::FlatMap<
    std::slice::Iter<'a, Arc<Vec<T>>>,
    std::slice::Iter<'a, T>,
    fn(&'a Arc<Vec<T>>) -> std::slice::Iter<'a, T>,
>;
impl<T> SharedChunks<T> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn storage_id(&self) -> usize {
        Arc::as_ptr(&self.chunks) as usize
    }
    pub fn head_bytes(&self) -> usize {
        self.chunks.capacity() * std::mem::size_of::<Arc<Vec<T>>>()
    }
    pub fn chunks(&self) -> impl Iterator<Item = (usize, usize, &[T])> {
        self.chunks.iter().map(|chunk| {
            (
                Arc::as_ptr(chunk) as usize,
                chunk.capacity() * std::mem::size_of::<T>(),
                chunk.as_slice(),
            )
        })
    }
    pub fn iter(&self) -> ChunkIter<'_, T> {
        self.chunks.iter().flat_map(elements::<T>)
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        (index < self.len).then(|| &self.chunks[index / CHUNK][index % CHUNK])
    }
    pub fn last(&self) -> Option<&T> {
        self.chunks.last().and_then(|c| c.last())
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
impl<T: Clone> SharedChunks<T> {
    pub fn push(&mut self, item: T) {
        self.brush_summary.take();
        let chunks = Arc::make_mut(&mut self.chunks);
        if self.len.is_multiple_of(CHUNK) {
            chunks.push(Arc::new(Vec::with_capacity(CHUNK)));
        }
        Arc::make_mut(chunks.last_mut().expect("tail chunk")).push(item);
        self.len += 1;
    }
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.brush_summary.take();
        Arc::make_mut(&mut self.chunks)
            .last_mut()
            .and_then(|c| Arc::make_mut(c).last_mut())
    }
}
impl<T: PartialEq> SharedChunks<T> {
    pub fn has_prefix(&self, other: &Self) -> bool {
        if self.len < other.len {
            return false;
        }
        for (i, old) in other.chunks.iter().enumerate() {
            let Some(new) = self.chunks.get(i) else {
                return false;
            };
            if !Arc::ptr_eq(new, old) && !new.starts_with(old) {
                return false;
            }
        }
        true
    }
}
impl<T: PartialEq> PartialEq for SharedChunks<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && (Arc::ptr_eq(&self.chunks, &other.chunks) || self.has_prefix(other))
    }
}
impl<T> Index<usize> for SharedChunks<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index).expect("stroke index")
    }
}
impl<T: Clone> IndexMut<usize> for SharedChunks<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        self.brush_summary.take();
        assert!(index < self.len);
        &mut Arc::make_mut(&mut Arc::make_mut(&mut self.chunks)[index / CHUNK])[index % CHUNK]
    }
}
impl<T: Clone> From<Vec<T>> for SharedChunks<T> {
    fn from(value: Vec<T>) -> Self {
        value.into_iter().collect()
    }
}
impl<T: Clone> FromIterator<T> for SharedChunks<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut result = Self::default();
        for item in iter {
            result.push(item);
        }
        result
    }
}
impl<'a, T> IntoIterator for &'a SharedChunks<T> {
    type Item = &'a T;
    type IntoIter = ChunkIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Clone> IntoIterator for SharedChunks<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().cloned().collect::<Vec<_>>().into_iter()
    }
}
impl<T: Serialize> Serialize for SharedChunks<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for item in self {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}
impl<'de, T: Deserialize<'de> + Clone> Deserialize<'de> for SharedChunks<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer).map(Into::into)
    }
}

impl SharedChunks<super::Stroke> {
    pub(super) fn brush_mask(&self) -> u8 {
        *self.brush_summary.get_or_init(|| {
            self.iter()
                .fold(0, |mask, stroke| mask | (1 << stroke.brush as u8))
        })
    }
}
