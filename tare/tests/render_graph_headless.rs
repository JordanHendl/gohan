use dashi::*;
use tare::graph::*;

#[test]
fn headless_render_graph_executes_without_validation_noise() {
    // Ensure validation layers stay disabled so the test output remains quiet.
    unsafe {
        std::env::set_var("DASHI_VALIDATION", "0");
    }

    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut graph = RenderGraph::new(&mut context);

    let vertex = graph.make_buffer(&BufferInfo::default());
    let indices = graph.make_buffer(&BufferInfo::default());

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
