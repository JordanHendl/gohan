#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirtyRange {
    start: u32,
    end: u32,
    dirty: bool,
}

impl DirtyRange {
    pub fn mark_bytes(&mut self, offset: u32, len: u32) {
        if len == 0 {
            return;
        }

        let end = offset.saturating_add(len);
        if !self.dirty {
            self.start = offset;
            self.end = end;
            self.dirty = true;
            return;
        }

        self.start = self.start.min(offset);
        self.end = self.end.max(end);
    }

    pub fn mark_elements<T>(&mut self, start: usize, count: usize) {
        let size = std::mem::size_of::<T>() as u32;
        let offset = start as u32 * size;
        let len = count as u32 * size;
        self.mark_bytes(offset, len);
    }

    pub fn take(&mut self) -> Option<(u32, u32)> {
        if !self.dirty {
            return None;
        }

        self.dirty = false;
        Some((self.start, self.end))
    }
}
