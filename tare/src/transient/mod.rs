use std::collections::HashMap;

use dashi::*;
use driver::command::BeginRenderPass;

pub struct Ring<T, const N: usize> {
    current: usize,
    data: [T; N],
}

impl<T: Default, const N: usize> Ring<T, N> {
    pub fn new(&mut self) -> Self {
        todo!()
    }

    pub fn advance(&mut self) {
        todo!()
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn data(&self) -> &T {
        &self.data[self.current]
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data[self.current]
    }
}

const MAX_FRAMES: usize = 3;
pub struct TransientAllocator {
    images: Ring<HashMap<Handle<Image>, ImageInfo<'static>>, MAX_FRAMES>,
    buffers: Ring<HashMap<Handle<Buffer>, BufferInfo<'static>>, MAX_FRAMES>,
    renderpasses: Ring<HashMap<Handle<RenderPass>, RenderPassInfo<'static>>, MAX_FRAMES>,
}

impl TransientAllocator {
    pub fn new(ctx: &mut Context) -> Self {
        todo!()
    }

    // Helper function to check for stale data and remove it.
    fn check_for_stale(&mut self) {
        todo!()
    }

    pub fn advance(&mut self) {
        // advance
        self.check_for_stale();
        todo!()
    }

    // Make a transient image matching the parameters input from this frame.
    pub fn make_image(&mut self, info: &ImageInfo) -> ImageView {
        todo!()
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        todo!()
    }

    pub fn make_render_pass(&mut self, info: &RenderPassInfo) -> Handle<RenderPass> {
        todo!()
    }
}
