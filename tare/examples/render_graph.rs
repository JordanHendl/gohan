use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use dashi::*;
use driver::command::{BlitImage, DrawIndexed};
use tare::graph::*;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::platform::run_return::EventLoopExtRunReturn;

#[repr(C)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

fn main() {
    let mut context = Context::new(&Default::default()).unwrap();
    let mut graph = RenderGraph::new(&mut context);
    let compiler = Compiler::new().expect("create bento compiler");

    let vert_source = r#"
        #version 450 core
        layout(location = 0) in vec2 in_pos;
        layout(location = 1) in vec3 in_color;
        layout(location = 0) out vec3 v_color;

        void main() {
            v_color = in_color;
            gl_Position = vec4(in_pos, 0.0, 1.0);
        }
    "#;

    let frag_source = r#"
        #version 450 core
        layout(location = 0) in vec3 v_color;
        layout(location = 0) out vec4 out_color;

        void main() {
            out_color = vec4(v_color, 1.0);
        }
    "#;

    let vert_shader = compiler
        .compile(
            vert_source.as_bytes(),
            &Request {
                name: Some("render_graph_example_vert".to_string()),
                lang: ShaderLang::Glsl,
                stage: ShaderType::Vertex,
                optimization: OptimizationLevel::None,
                debug_symbols: false,
                ..Default::default()
            },
        )
        .expect("compile vertex shader");

    let frag_shader = compiler
        .compile(
            frag_source.as_bytes(),
            &Request {
                name: Some("render_graph_example_frag".to_string()),
                lang: ShaderLang::Glsl,
                stage: ShaderType::Fragment,
                optimization: OptimizationLevel::None,
                debug_symbols: false,
                ..Default::default()
            },
        )
        .expect("compile fragment shader");

    let vertices = vec![
        Vertex {
            position: [-0.8, -0.8],
            color: [1.0, 0.2, 0.3],
        },
        Vertex {
            position: [0.8, -0.8],
            color: [0.2, 1.0, 0.4],
        },
        Vertex {
            position: [0.0, 0.8],
            color: [0.2, 0.4, 1.0],
        },
    ];

    let indices: [u32; 3] = [0, 1, 2];

    let vertex = graph.make_buffer(&BufferInfo {
        debug_name: "render-graph-vertices",
        byte_size: (std::mem::size_of::<Vertex>() * vertices.len()) as u32,
        visibility: MemoryVisibility::Gpu,
        usage: BufferUsage::VERTEX,
        initial_data: unsafe { Some(vertices.align_to::<u8>().1) },
        ..Default::default()
    });

    let indices = graph.make_buffer(&BufferInfo {
        debug_name: "render-graph-indices",
        byte_size: (std::mem::size_of::<u32>() * indices.len()) as u32,
        visibility: MemoryVisibility::Gpu,
        usage: BufferUsage::INDEX,
        initial_data: unsafe { Some(indices.align_to::<u8>().1) },
        ..Default::default()
    });

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [1024, 1024, 1],
        format: Format::RGBA8,
        ..Default::default()
    });

    let p_layout = context
        .make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
            vertex_info: VertexDescriptionInfo {
                entries: &[
                    VertexEntryInfo {
                        format: ShaderPrimitiveType::Vec2,
                        location: 0,
                        offset: 0,
                    },
                    VertexEntryInfo {
                        format: ShaderPrimitiveType::Vec3,
                        location: 1,
                        offset: 8,
                    },
                ],
                stride: std::mem::size_of::<Vertex>(),
                rate: VertexRate::Vertex,
            },
            bg_layouts: [None, None, None, None],
            bt_layouts: [None, None, None, None],
            shaders: &[
                PipelineShaderInfo {
                    stage: ShaderType::Vertex,
                    spirv: vert_shader.spirv.as_slice(),
                    specialization: &[],
                },
                PipelineShaderInfo {
                    stage: ShaderType::Fragment,
                    spirv: frag_shader.spirv.as_slice(),
                    specialization: &[],
                },
            ],
            details: GraphicsPipelineDetails::default(),
            debug_name: "render-graph-pipeline-layout",
        })
        .expect("Make Pipeline Layout");

    let pso = context
        .make_graphics_pipeline(&GraphicsPipelineInfo {
            debug_name: "render-graph-pipeline",
            layout: p_layout,
            attachment_formats: vec![Format::RGBA8],
            depth_format: None,
            subpass_samples: SubpassSampleInfo {
                color_samples: vec![SampleCount::default()],
                depth_sample: None,
            },
            subpass_id: 0,
        })
        .expect("Make graphics pipeline");

    let mut display = context
        .make_display(&DisplayInfo {
            window: WindowInfo {
                title: "render-graph example".to_string(),
                size: [1024, 1024],
                resizable: false,
            },
            ..Default::default()
        })
        .expect("Make display");

    let viewport = Viewport {
        area: FRect2D {
            w: 1024.0,
            h: 1024.0,
            ..Default::default()
        },
        scissor: Rect2D {
            w: 1024,
            h: 1024,
            ..Default::default()
        },
        ..Default::default()
    };

    let draw_pass = SubpassInfo {
        viewport,
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

    let sems = context.make_semaphores(1).unwrap();
    'running: loop {
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

            if should_exit {
                break 'running;
            }
        }
        // Get the next image from the display.
        let (img, sem, _idx, _good) = context.acquire_new_image(&mut display).unwrap();

        graph.add_subpass(&draw_pass, move |stream| {
            let mut s = stream.bind_graphics_pipeline(pso);

            s.draw_indexed(&DrawIndexed {
                vertices: vertex.handle,
                indices: indices.handle,
                index_count: 3,
                ..Default::default()
            });

            s.blit_images(&BlitImage {
                src: target.img,
                dst: img.img,
                ..Default::default()
            });
            s.prepare_for_presentation(img.img);

            s.unbind_graphics_pipeline()
        });

        graph.execute_with(&SubmitInfo {
            wait_sems: &[sem],
            signal_sems: &[sems[0]],
        });

        context.present_display(&display, &[sems[0]]).unwrap();
    }
}
