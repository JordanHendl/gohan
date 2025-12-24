use dashi::driver::command::CopyImageBuffer;
use dashi::*;
use tare::graph::*;
use tare::transient::TransientAllocator;

#[test]
fn headless_render_graph_executes_without_validation_noise() {
    // Ensure validation layers stay disabled so the test output remains quiet.
    unsafe {
        std::env::set_var("DASHI_VALIDATION", "0");
    }

    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut graph = RenderGraph::new(&mut context);

    let _vertex = graph.make_buffer(&BufferInfo::default());
    let _indices = graph.make_buffer(&BufferInfo::default());

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [16, 16, 1],
        ..Default::default()
    });

    let subpass = SubpassInfo {
        viewport: Viewport::default(),
        color_attachments: [Some(target), None, None, None, None, None, None, None],
        depth_attachment: None,
        clear_values: [
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0])),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        depth_clear: None,
    };

    graph.add_subpass(&subpass, |stream| stream);

    let render_pass = graph
        .render_pass_handle()
        .expect("render pass to be created for headless graph");

    assert_ne!(render_pass, Handle::<RenderPass>::default());
}

#[test]
fn headless_render_graph_outputs_readable_image() {
    // Ensure validation layers stay disabled so the test output remains quiet.
    unsafe {
        std::env::set_var("DASHI_VALIDATION", "0");
    }

    const WIDTH: u32 = 8;
    const HEIGHT: u32 = 8;
    const EXPECTED_COLOR: [u8; 4] = [255, 0, 0, 255];

    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut graph = RenderGraph::new(&mut context);

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [WIDTH, HEIGHT, 1],
        format: Format::RGBA8,
        ..Default::default()
    });

    graph.add_subpass(
        &SubpassInfo {
            viewport: Viewport::default(),
            color_attachments: [Some(target), None, None, None, None, None, None, None],
            depth_attachment: None,
            clear_values: [
                Some(ClearValue::Color([
                    EXPECTED_COLOR[0] as f32 / 255.0,
                    EXPECTED_COLOR[1] as f32 / 255.0,
                    EXPECTED_COLOR[2] as f32 / 255.0,
                    EXPECTED_COLOR[3] as f32 / 255.0,
                ])),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            depth_clear: None,
        },
        |stream| stream,
    );

    graph.execute();

    let readback = context
        .make_buffer(&BufferInfo {
            debug_name: "[READBACK]",
            byte_size: WIDTH * HEIGHT * 4,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::ALL,
            ..Default::default()
        })
        .expect("create readback buffer");

    let mut copy_ring = context
        .make_command_ring(&CommandQueueInfo2 {
            debug_name: "render-graph-readback",
            parent: None,
            queue_type: QueueType::Graphics,
        })
        .expect("create command ring for readback");

    copy_ring
        .record(|cmd| {
            let stream = CommandStream::new()
                .begin()
                .copy_image_to_buffer(&CopyImageBuffer {
                    src: target.img,
                    dst: readback,
                    range: SubresourceRange::default(),
                    dst_offset: 0,
                })
                .end();
            stream.append(cmd);
        })
        .expect("record readback commands");

    copy_ring
        .submit(&SubmitInfo::default())
        .expect("submit readback commands");

    copy_ring.wait_all().expect("wait for readback");

    let data = context
        .map_buffer::<u8>(readback.into())
        .expect("map readback buffer")
        .to_vec();
    context
        .unmap_buffer(readback)
        .expect("unmap readback buffer");

    assert_eq!(data.len() as u32, WIDTH * HEIGHT * 4);
    for chunk in data.chunks_exact(4) {
        assert_eq!(chunk, EXPECTED_COLOR);
    }
}

#[test]
fn render_graph_can_reuse_allocator() {
    unsafe {
        std::env::set_var("DASHI_VALIDATION", "0");
    }

    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut allocator = TransientAllocator::new(&mut context);

    let mut graph = RenderGraph::new_with_transient_allocator(&mut context, &mut allocator);

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [4, 4, 1],
        ..Default::default()
    });

    graph.add_subpass(
        &SubpassInfo {
            viewport: Viewport::default(),
            color_attachments: [Some(target), None, None, None, None, None, None, None],
            depth_attachment: None,
            clear_values: [
                Some(ClearValue::Color([1.0, 0.0, 0.0, 1.0])),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            depth_clear: None,
        },
        |stream| stream,
    );

    graph.execute();

    // Executing successfully proves the external allocator can be used without crashing.
}
