use cmd::{CommandStream, PendingGraphics};
use dashi::{execution::CommandRing, *};
use driver::command::BeginRenderPass;

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
    alloc: TransientAllocator,
    ring: CommandRing,
    subpasses: Vec<StoredSubpass>,
    cached_render_passes: Vec<Handle<RenderPass>>,
    cached_begins: Vec<BeginRenderPass>,
}

struct StoredSubpass {
    info: SubpassInfo,
    cb: Box<dyn FnMut(CommandStream<PendingGraphics>) -> CommandStream<PendingGraphics>>,
}

impl RenderGraph {
    pub fn new(ctx: &mut Context) -> Self {
        let ring = ctx
            .make_command_ring(&CommandQueueInfo2 {
                debug_name: "tare-render-graph",
                parent: None,
                queue_type: QueueType::Graphics,
            })
            .expect("Create command ring for render graph");
        Self {
            alloc: TransientAllocator::new(ctx),
            ring,
            subpasses: Vec::new(),
            cached_render_passes: Vec::new(),
            cached_begins: Vec::new(),
        }
    }

    // Make a transient image matching the parameters input.
    pub fn make_image(&mut self, info: &ImageInfo) -> ImageView {
        self.alloc.make_image(info)
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        self.alloc.make_buffer(info)
    }

    // Append a potential subpass
    pub fn add_subpass<F>(&mut self, info: &SubpassInfo, cb: F)
    where
        F: FnMut(CommandStream<PendingGraphics>) -> CommandStream<PendingGraphics> + 'static,
    {
        self.subpasses.push(StoredSubpass {
            info: info.clone(),
            cb: Box::new(cb),
        });
        self.cached_render_passes.clear();
        self.cached_begins.clear();
    }

    pub fn render_pass_handle(&mut self) -> Option<Handle<RenderPass>> {
        self.solve_and_cache()
            .and_then(|(rps, _)| rps.into_iter().next())
    }

    fn solve_and_cache(&mut self) -> Option<(Vec<Handle<RenderPass>>, Vec<BeginRenderPass>)> {
        if !(self.cached_render_passes.is_empty() || self.cached_begins.is_empty()) {
            return Some((
                self.cached_render_passes.clone(),
                self.cached_begins.clone(),
            ));
        }

        if self.subpasses.is_empty() {
            return None;
        }

        self.cached_render_passes.clear();
        self.cached_begins.clear();

        for subpass in &self.subpasses {
            let mut colors = Vec::new();
            for _attachment in subpass.info.color_attachments.iter().flatten().take(4) {
                let mut desc = AttachmentDescription::default();
                // Keep load_op aligned with whether we intend to clear the attachment.
                let clear = subpass.info.clear_values[colors.len()].is_some();
                desc.load_op = if clear { LoadOp::Clear } else { LoadOp::Load };
                colors.push(desc);
            }

            let depth_desc = subpass.info.depth_attachment.map(|_| {
                let mut desc = AttachmentDescription::default();
                desc.format = Format::D24S8;
                desc.load_op = if subpass.info.depth_clear.is_some() {
                    LoadOp::Clear
                } else {
                    LoadOp::Load
                };
                desc
            });

            let subpass_description = SubpassDescription {
                color_attachments: colors.as_slice(),
                depth_stencil_attachment: depth_desc.as_ref(),
                subpass_dependencies: &[],
            };

            let rp_info = RenderPassInfo {
                debug_name: "tare-render-graph-pass",
                viewport: subpass.info.viewport,
                subpasses: std::slice::from_ref(&subpass_description),
            };

            let render_pass = self.alloc.make_render_pass(&rp_info);

            let mut begin = BeginRenderPass {
                viewport: subpass.info.viewport,
                render_pass,
                color_attachments: [None; 4],
                depth_attachment: subpass.info.depth_attachment,
                clear_values: [None; 4],
            };

            for i in 0..4 {
                begin.color_attachments[i] = subpass.info.color_attachments[i];
                begin.clear_values[i] = subpass.info.clear_values[i];
            }

            self.cached_render_passes.push(render_pass);
            self.cached_begins.push(begin);
        }

        Some((
            self.cached_render_passes.clone(),
            self.cached_begins.clone(),
        ))
    }

    pub fn execute(&mut self) {
        self.execute_with(&Default::default());
    }

    pub fn execute_with(&mut self, info: &SubmitInfo) {
        let Some((_, begin_entries)) = self.solve_and_cache() else {
            return;
        };

        self.ring
            .record(|cmd| {
                let mut stream = CommandStream::new().begin();
                for (subpass, begin) in self.subpasses.iter_mut().zip(begin_entries.iter()) {
                    let mut subpass_stream = stream.begin_render_pass(begin);
                    subpass_stream = (subpass.cb)(subpass_stream);
                    stream = subpass_stream.stop_drawing();
                }
                stream.end().append(cmd);
            })
            .expect("Failed to record render graph commands");

        self.ring
            .submit(info)
            .expect("Failed to submit render graph commands");

        // Advance transient allocator
        self.alloc.advance();
        self.subpasses.clear();
        self.cached_render_passes.clear();
        self.cached_begins.clear();
    }
}
