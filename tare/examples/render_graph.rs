use dashi::*;
use driver::command::{BlitImage, DrawIndexed};
use tare::graph::*;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::platform::run_return::EventLoopExtRunReturn;

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

    let p_layout = context
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
        .make_graphics_pipeline(&GraphicsPipelineInfo {
            debug_name: "dbg",
            layout: p_layout,
            attachment_formats: vec![Format::RGBA8],
            depth_format: None,
            subpass_samples: SubpassSampleInfo {
                color_samples: vec![SampleCount::default()],
                depth_sample: None,
            },
            subpass_id: 0
        })
        .expect("Make graphics pipeline");

    let mut display = context
        .make_display(&DisplayInfo {
            window: WindowInfo {
                title: "example".to_string(),
                size: [1024, 1024],
                resizable: false,
            },
            ..Default::default()
        })
        .expect("Make display");

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

    let sems = context.make_semaphores(1).unwrap();
    loop {
        {
            let mut should_exit = false;
            let event_loop = display.winit_event_loop();
            event_loop.run_return(|event, _target, control_flow| {
                *control_flow = ControlFlow::Exit;
                if let Event::WindowEvent { event, .. } = event {
                    match event {
                        WindowEvent::CloseRequested => should_exit = true,
                        WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    state: ElementState::Pressed,
                                    ..
                                },
                            ..
                        } => should_exit = true,
                        _ => {}
                    }
                }
            });
        }
        // Get the next image from the display.
        let (img, sem, _idx, _good) = context.acquire_new_image(&mut display).unwrap();

        for (i, subpass) in subpasses.iter().enumerate() {
            graph.add_subpass(subpass, move |stream| {
                let mut s = stream.bind_graphics_pipeline(pso);

                s.draw_indexed(&DrawIndexed {
                    vertices: vertex.handle,
                    indices: indices.handle,
                    ..Default::default()
                });

                if i == 1 {
                    s.blit_images(&BlitImage {
                        src: target.img,
                        dst: img.img,
                        ..Default::default()
                    });
                    s.prepare_for_presentation(img.img);
                }

                return s.unbind_graphics_pipeline();
            });
        }

        graph.execute_with(&SubmitInfo {
            wait_sems: &[sem],
            signal_sems: &[sems[0]],
        });
        
        
        context.present_display(&display, &[sems[0]]).unwrap();
    }
}
