use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use dashi::execution::command_dispatch::*;
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

    let vertex = context
        .make_buffer(&BufferInfo {
            debug_name: "render-graph-vertices",
            byte_size: (std::mem::size_of::<Vertex>() * vertices.len()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::VERTEX,
            initial_data: unsafe { Some(vertices.align_to::<u8>().1) },
            ..Default::default()
        })
        .unwrap();

    let indices = context
        .make_buffer(&BufferInfo {
            debug_name: "render-graph-indices",
            byte_size: (std::mem::size_of::<u32>() * indices.len()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::INDEX,
            initial_data: unsafe { Some(indices.align_to::<u8>().1) },
            ..Default::default()
        })
        .unwrap();
    
    let pso = bento::builder::PSOBuilder::new()
        .vertex_compiled(Some(vert_shader))
        .fragment_compiled(Some(frag_shader))
        .build(&mut context).unwrap();

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

    CommandDispatch::init(&mut context).unwrap();

    let mut graph = RenderGraph::new(&mut context);
    'running: loop {
        let sems = graph.make_semaphores(2);
        let target = graph.make_image(&ImageInfo {
            debug_name: "[ATTACHMENT]",
            dim: [1024, 1024, 1],
            format: Format::RGBA8,
            ..Default::default()
        });
        let draw_pass = SubpassInfo {
            viewport,
            color_attachments: fill![Some(target.view); None; 8],
            depth_attachment: None,
            clear_values: fill![Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0])), None; None; 8],
            depth_clear: None,
        };

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
            stream
                .bind_graphics_pipeline(pso.handle)
                .update_viewport(&viewport)
                .draw_indexed(&DrawIndexed {
                    vertices: vertex.into(),
                    indices: indices.into(),
                    index_count: 3,
                    ..Default::default()
                })
                .unbind_graphics_pipeline()
        });

        graph.execute_with(&SubmitInfo {
            wait_sems: &[sem],
            signal_sems: &[sems[0]],
        });

        let cmd = CommandStream::new()
            .begin()
            .blit_images(&BlitImage {
                src: target.view.img,
                dst: img.img,
                ..Default::default()
            })
            .prepare_for_presentation(img.img)
            .end();

        CommandDispatch::dispatch(
            cmd,
            &SubmitInfo2 {
                wait_sems: fill![
                    sems[0];
                    Default::default();
                    4
                ],
                signal_sems: fill![
                    sems[1];
                    Default::default();
                    4
                ],
            },
        )
        .expect("Failed to dispatch command!");

        context.present_display(&display, &[sems[1]]).unwrap();

        CommandDispatch::tick().expect("Failed to tick Command Dispatch!");
    }
}
