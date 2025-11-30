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
    cached_render_pass: Option<Handle<RenderPass>>,
    cached_begin: Option<BeginRenderPass>,
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
            cached_render_pass: None,
            cached_begin: None,
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
        self.cached_render_pass = None;
        self.cached_begin = None;
    }

    pub fn render_pass_handle(&mut self) -> Option<Handle<RenderPass>> {
        self.solve_and_cache().map(|(rp, _)| rp)
    }

    fn solve_and_cache(&mut self) -> Option<(Handle<RenderPass>, BeginRenderPass)> {
        if let (Some(rp), Some(begin)) = (self.cached_render_pass, self.cached_begin) {
            return Some((rp, begin));
        }

        if self.subpasses.is_empty() {
            return None;
        }

        let mut color_storage: Vec<Vec<AttachmentDescription>> = Vec::new();
        let mut depth_storage: Vec<Option<AttachmentDescription>> = Vec::new();

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

            color_storage.push(colors);
            depth_storage.push(depth_desc);
        }

        let subpass_descriptions: Vec<SubpassDescription<'_>> = color_storage
            .iter()
            .zip(depth_storage.iter())
            .map(|(colors, depth)| SubpassDescription {
                color_attachments: colors.as_slice(),
                depth_stencil_attachment: depth.as_ref(),
                subpass_dependencies: &[],
            })
            .collect();

        let viewport = self
            .subpasses
            .first()
            .map(|sp| sp.info.viewport)
            .unwrap_or_default();

        let rp_info = RenderPassInfo {
            debug_name: "tare-render-graph-pass",
            viewport,
            subpasses: &subpass_descriptions,
        };

        let render_pass = self.alloc.make_render_pass(&rp_info);

        let mut begin = BeginRenderPass {
            viewport,
            render_pass,
            color_attachments: [None; 4],
            depth_attachment: None,
            clear_values: [None; 4],
        };

        let first = &self.subpasses[0].info;
        begin.depth_attachment = first.depth_attachment;
        for i in 0..4 {
            begin.color_attachments[i] = first.color_attachments[i];
            begin.clear_values[i] = first.clear_values[i];
        }

        self.cached_render_pass = Some(render_pass);
        self.cached_begin = Some(begin);

        Some((render_pass, begin))
    }

    pub fn execute(&mut self) {
        let Some((_render_pass, begin)) = self.solve_and_cache() else {
            return;
        };

        self.ring
            .record(|cmd| {
                let mut stream = CommandStream::new().begin();
                let mut subpass_stream = stream.begin_render_pass(&begin);
                let subpass_count = self.subpasses.len();

                for (idx, subpass) in self.subpasses.iter_mut().enumerate() {
                    subpass_stream = (subpass.cb)(subpass_stream);
                    if idx + 1 < subpass_count {
                        subpass_stream.next_subpass();
                    }
                }

                stream = subpass_stream.stop_drawing();
                stream.end().append(cmd);
            })
            .expect("Failed to record render graph commands");

        self.ring
            .submit(&SubmitInfo::default())
            .expect("Failed to submit render graph commands");

        // Advance transient allocator
        self.alloc.advance();
        self.subpasses.clear();
        self.cached_render_pass = None;
        self.cached_begin = None;
    }
}
