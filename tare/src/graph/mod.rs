use std::ptr::NonNull;

use cmd::{CommandStream, Executable, PendingGraphics, Recording};
use dashi::{execution::CommandRing, *};
use driver::command::BeginRenderPass;

use crate::transient::{BindlessTextureRegistry, TransientAllocator, TransientImage};

#[derive(Default, Debug, Clone)]
pub struct SubpassInfo {
    pub viewport: Viewport,
    pub color_attachments: [Option<ImageView>; 8],
    pub depth_attachment: Option<ImageView>,
    pub clear_values: [Option<ClearValue>; 8],
    pub depth_clear: Option<ClearValue>,
}

pub struct RenderGraph {
    alloc: TransientAllocatorOwner,
    ring: CommandRing,
    passes: Vec<GraphPass>,
    cached_render_passes: Vec<Handle<RenderPass>>,
    cached_begins: Vec<BeginRenderPass>,
}

enum TransientAllocatorOwner {
    Owned(TransientAllocator),
    Borrowed(NonNull<TransientAllocator>),
}

impl TransientAllocatorOwner {
    fn owned(ctx: &mut Context) -> Self {
        Self::Owned(TransientAllocator::new(ctx))
    }

    fn borrowed(alloc: &mut TransientAllocator) -> Self {
        Self::Borrowed(NonNull::from(alloc))
    }

    fn as_mut(&mut self) -> &mut TransientAllocator {
        match self {
            Self::Owned(alloc) => alloc,
            Self::Borrowed(alloc) => unsafe { alloc.as_mut() },
        }
    }
}

struct StoredSubpass {
    info: SubpassInfo,
    cb: Box<dyn FnMut(CommandStream<PendingGraphics>) -> CommandStream<PendingGraphics>>,
}

struct StoredComputePass {
    cb: Box<dyn FnMut(CommandStream<Recording>) -> CommandStream<Executable>>,
}

enum GraphPass {
    Render(StoredSubpass),
    Compute(StoredComputePass),
}

impl RenderGraph {
    pub fn new(ctx: &mut Context) -> Self {
        Self::with_transient_allocator(ctx, None)
    }

    pub fn new_with_transient_allocator(
        ctx: &mut Context,
        allocator: &mut TransientAllocator,
    ) -> Self {
        Self::with_transient_allocator(ctx, Some(allocator))
    }

    pub fn new_with_bindless_registry(
        ctx: &mut Context,
        registry: &mut impl BindlessTextureRegistry,
    ) -> Self {
        let mut graph = Self::with_transient_allocator(ctx, None);
        graph.set_bindless_registry(registry);
        graph
    }

    fn with_transient_allocator(
        ctx: &mut Context,
        allocator: Option<&mut TransientAllocator>,
    ) -> Self {
        let ring = ctx
            .make_command_ring(&CommandQueueInfo2 {
                debug_name: "tare-render-graph",
                parent: None,
                queue_type: QueueType::Graphics,
            })
            .expect("Create command ring for render graph");
        Self {
            alloc: allocator
                .map(TransientAllocatorOwner::borrowed)
                .unwrap_or_else(|| TransientAllocatorOwner::owned(ctx)),
            ring,
            passes: Vec::new(),
            cached_render_passes: Vec::new(),
            cached_begins: Vec::new(),
        }
    }

    pub fn make_semaphore(&mut self) -> Handle<Semaphore> {
        self.alloc.as_mut().make_semaphore()
    }

    pub fn make_semaphores(&mut self, count: usize) -> Vec<Handle<Semaphore>> {
        self.alloc.as_mut().make_semaphores(count)
    }

    // Make a transient image matching the parameters input.
    pub fn make_image(&mut self, info: &ImageInfo) -> TransientImage {
        self.alloc.as_mut().make_image(info)
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        self.alloc.as_mut().make_buffer(info)
    }

    pub fn set_bindless_registry(&mut self, registry: &mut impl BindlessTextureRegistry) {
        self.alloc.as_mut().set_bindless_registry(registry);
    }

    // Append a potential subpass
    pub fn add_subpass<F>(&mut self, info: &SubpassInfo, cb: F)
    where
        F: FnMut(CommandStream<PendingGraphics>) -> CommandStream<PendingGraphics>,
    {
        let cb = unsafe {
            let raw: *mut F = Box::into_raw(Box::new(cb));
            Box::from_raw(
                raw as *mut dyn FnMut(
                    CommandStream<PendingGraphics>,
                ) -> CommandStream<PendingGraphics>,
            )
        };
        self.passes.push(GraphPass::Render(StoredSubpass {
            info: info.clone(),
            cb,
        }));
        self.cached_render_passes.clear();
        self.cached_begins.clear();
    }

    pub fn add_compute_pass<F>(&mut self, cb: F)
    where
        F: FnMut(CommandStream<Recording>) -> CommandStream<Executable>,
    {
        let cb = unsafe {
            let raw: *mut F = Box::into_raw(Box::new(cb));
            Box::from_raw(
                raw as *mut dyn FnMut(CommandStream<Recording>) -> CommandStream<Executable>,
            )
        };
        self.passes
            .push(GraphPass::Compute(StoredComputePass { cb }));
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

        if self.passes.is_empty() {
            return None;
        }

        self.cached_render_passes.clear();
        self.cached_begins.clear();

        for pass in &self.passes {
            let GraphPass::Render(subpass) = pass else {
                continue;
            };

            let mut colors = Vec::new();
            for attachment in subpass.info.color_attachments.iter().flatten().take(4) {
                let mut desc = AttachmentDescription::default();
                // Keep load_op aligned with whether we intend to clear the attachment.
                let clear = subpass.info.clear_values[colors.len()].is_some();
                let info = self.alloc.as_mut().context().image_info(attachment.img);
                desc.samples = info.samples;
                desc.format = info.format;
                desc.load_op = if clear { LoadOp::Clear } else { LoadOp::Load };
                colors.push(desc);
            }

            let depth_desc = subpass.info.depth_attachment.map(|attach| {
                let mut desc = AttachmentDescription::default();
                let info = self.alloc.as_mut().context().image_info(attach.img);
                desc.samples = info.samples;
                desc.format = info.format;
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

            let render_pass = self.alloc.as_mut().make_render_pass(&rp_info);

            let mut begin = BeginRenderPass {
                viewport: subpass.info.viewport,
                render_pass,
                color_attachments: [None; 4],
                depth_attachment: subpass.info.depth_attachment,
                clear_values: [None; 4],
                depth_clear: subpass.info.depth_clear,
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

        let mut begin_iter = begin_entries.iter();

        self.ring
            .record(|cmd| {
                for pass in self.passes.iter_mut() {
                    match pass {
                        GraphPass::Render(subpass) => {
                            let begin = begin_iter
                                .next()
                                .expect("begin entry should exist for every render pass");
                            let mut stream = CommandStream::new().begin();
                            let mut subpass_stream = stream.begin_render_pass(begin);
                            subpass_stream = (subpass.cb)(subpass_stream);
                            stream = subpass_stream.stop_drawing();
                            stream.end().append(cmd).unwrap();
                        }
                        GraphPass::Compute(compute) => {
                            let stream = CommandStream::new().begin();
                            let stream = (compute.cb)(stream);
                            stream.append(cmd).unwrap();
                        }
                    }
                }
            })
            .expect("Failed to record render graph commands");

        self.ring
            .submit(info)
            .expect("Failed to submit render graph commands");

        // Advance transient allocator
        self.alloc.as_mut().advance();
        self.passes.clear();
        self.cached_render_passes.clear();
        self.cached_begins.clear();
    }
}
