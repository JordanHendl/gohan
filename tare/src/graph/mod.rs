use std::ptr::NonNull;

use cmd::{Graphics, PendingGraphics};
use dashi::*;

use crate::transient::TransientAllocator;

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
    alloc: TransientAllocator,
}

impl RenderGraph {
    pub fn new(ctx: &mut Context) -> Self {
        todo!()
    }

    // Make a transient image matching the parameters input.
    pub fn make_image(&mut self, info: &ImageInfo) -> ImageView {
        todo!("Use transient allocator to make image")
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        todo!("Use transient allocator to make buffer")
    }

    // Append a potential subpass
    pub fn add_subpass<F>(&mut self, info: &SubpassInfo, mut cb: F)
    where
        F: FnMut(CommandStream<PendingGraphics>) -> CommandStream<PendingGraphics>,
    {
        todo!("Append subpass internally to solve later")
    }

    fn solve_and_cache(&mut self) {
        // Solve graph and cache it for future use
        todo!("If data is different: Solve graph, build render pass if needed based on subpasses. Cache result.");
        todo!("This is also the stage to resolve barriers, transitions, etc.")
    }

    pub fn execute(&mut self) {
        self.solve_and_cache();
        todo!("Start render pass");
        todo!("Iterate through subpasses, setting up command stream and executing callbacks, ");
        todo!("Collect command streams, execute");

        // Advance transient allocator
        self.alloc.advance();
        todo!()
    }
}
