use std::ptr::NonNull;

use cmd::{Graphics, PendingGraphics};
use dashi::*;

#[derive(Default, Debug, Clone)]
pub struct SubpassInfo {
    pub viewport: Viewport,
    pub color_attachments: [Option<ImageView>; 8],
    pub depth_attachment: Option<ImageView>,
    pub clear_values: [Option<ClearValue>; 8],
    pub depth_clear: Option<ClearValue>,
}

pub struct RenderGraph {
    ctx: NonNull<Context>,
}

impl RenderGraph {
    pub fn new(ctx: &mut Context) -> Self {
        todo!()
    }

    // Make a transient image matching the parameters input.
    pub fn make_image(&mut self, info: &ImageInfo) -> ImageView {
        todo!()
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        todo!()
    }

    // Append a potential subpass
    pub fn add_subpass<F>(&mut self, info: &SubpassInfo, mut cb: F)
    where
        F: FnMut(CommandStream<PendingGraphics>),
    {
        todo!()
    }

    // All transient data in this object is per-frame. This function advances the frame count to
    // use next frame's resources.
    fn advance_frame(&mut self) {}

    pub fn execute(&mut self) {
        self.advance_frame();
        todo!()
    }
}
