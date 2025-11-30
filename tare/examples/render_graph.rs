use dashi::*;
use driver::command::DrawIndexed;
use tare::graph::*;

fn main() {
    let mut context = Context::new(&Default::default()).unwrap();
    let mut graph = RenderGraph::new(&mut context);

    let vertex = graph.make_buffer(&BufferInfo::default());
    let indices = graph.make_buffer(&BufferInfo::default());

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [1024, 1024, 1],
        ..Default::default()
    });

    let _p_layout = context
        .make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
            debug_name: "example-pipeline",
            vertex_info: VertexDescriptionInfo {
                entries: &[],
                stride: 0,
                rate: VertexRate::Vertex,
            },
            bg_layouts: [None, None, None, None],
            bt_layouts: [None, None, None, None],
            shaders: &[],
            details: GraphicsPipelineDetails::default(),
        })
        .expect("Make Pipeline Layout");

    let pso = context
        .make_graphics_pipeline(&GraphicsPipelineInfo::default())
        .expect("Make graphics pipeline");

    let subpasses = vec![
        SubpassInfo {
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
        },
        SubpassInfo {
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
        },
    ];

    loop {
        for subpass in &subpasses {
            graph.add_subpass(subpass, move |stream| {
                let mut s = stream.bind_graphics_pipeline(pso);

                s.draw_indexed(&DrawIndexed {
                    vertices: vertex.handle,
                    indices: indices.handle,
                    ..Default::default()
                });

                return s.unbind_graphics_pipeline();
            });
        }

        graph.execute();
    }
}
