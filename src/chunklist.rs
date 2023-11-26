// A ChunkList is a 2-level Vec.
// - On one hand, it could be used to reduce the realloc overhead of Vec's capacity extension
// - On the other hand, it could be cheaply cloned so that we could take snapshots while the chunk
//   list is being pushed.

use std::cmp::{max, min};
use crate::consts::{CHUNK_LIST_INIT_CAPACITY, CHUNK_SIZE};
use std::sync::{Arc};
use parking_lot::Mutex;
use crate::spinlock::SpinLock;

pub type Chunk<T> = Arc<Vec<T>>;

struct ChunkListInner<T: Clone> {
    frozen: Vec<Chunk<T>>,
    pending: Chunk<T>,
    len: usize,
}

impl<T: Clone> ChunkListInner<T> {
    fn new() -> Self {
        ChunkListInner {
            frozen: Vec::with_capacity(CHUNK_LIST_INIT_CAPACITY),
            pending: Self::new_chunk(),
            len: 0,
        }
    }

    fn new_chunk() -> Chunk<T> {
        Arc::new(Vec::with_capacity(CHUNK_SIZE))
    }

    fn push(&mut self, item: T) {
        if self.pending.capacity() == self.pending.len() {
            self.frozen.push(self.pending.clone());
            self.pending = Self::new_chunk();
        }
        Arc::make_mut(&mut self.pending)
            .push(item);
        self.len += 1;
    }

    fn len(&self) -> usize {
        self.len
    }
}

pub struct ChunkList<T: Clone> {
    inner: Mutex<ChunkListInner<T>>,
}

impl<T: Clone> Default for ChunkList<T> {
    fn default() -> Self {
        ChunkList {
            inner: Mutex::new(ChunkListInner::new()),
        }
    }
}

impl<T: Clone> ChunkList<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, item: T) {
        let mut inner = self.inner.lock();
        inner.push(item);
    }

    pub fn append_vec(&self, vec: Vec<T>) {
        let mut inner = self.inner.lock();
        for item in vec.into_iter() {
            inner.push(item);
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        *inner = ChunkListInner::new();
    }

    pub fn snapshot(&self, start: usize) -> Vec<Chunk<T>> {
        let mut ret = Vec::new();
        let inner = self.inner.lock();

        let mut scanned = 0;
        for chunk in inner.frozen.iter() {
            if scanned > start {
                ret.push(chunk.clone());
            } else if scanned + chunk.len() > start {
                ret.push(Arc::new(Vec::from(&chunk[start - scanned..])))
            }
            scanned += chunk.len();
        }

        // copy the last chunk
        ret.push(Arc::new(Vec::from(&inner.pending[max(scanned, start) - scanned..])));
        ret
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.lock();
        inner.len()
    }
}

mod tests {
    use super::ChunkList;
    use crate::consts::CHUNK_SIZE;

    #[test]
    fn test_push() {
        let chunk_list = ChunkList::new();
        let size = CHUNK_SIZE + CHUNK_SIZE / 2;
        for i in 0..size {
            chunk_list.push(i);
        }
        assert_eq!(size, chunk_list.len());
    }
}
