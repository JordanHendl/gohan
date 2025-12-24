use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use dashi::driver::command::{BeginDrawing, DrawIndexed};
use dashi::{CommandStream, *};
use furikake::recipe::RecipeBook;
use furikake::*;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

fn main() {
    let mut ctx =
        gpu::Context::headless(&ContextInfo::default()).expect("Unable to make dashi context");

    // Compile simple shaders that use the reserved timing binding.
    let vert_source = r#"
        #version 450 core
        layout(location = 0) in vec2 in_pos;
        layout(location = 1) in vec3 in_color;
        layout(location = 0) out vec3 v_color;

        layout(binding = 0) uniform timing{
            float current_time_ms;
            float frame_time_ms;
        } meshi_timing;

        void main() {
            v_color = in_color + vec3(sin(meshi_timing.current_time_ms * 0.001));
            gl_Position = vec4(in_pos, 0.0, 1.0);
        }
    "#;

    let frag_source = r#"
        #version 450 core
        layout(location = 0) in vec3 v_color;
        layout(location = 0) out vec4 out_color;

        layout(binding = 0) uniform timing{
            float current_time_ms;
            float frame_time_ms;
        } meshi_timing;

        void main() {
            float fade = clamp(meshi_timing.frame_time_ms * 0.01, 0.0, 1.0);
            out_color = vec4(mix(v_color, vec3(0.2, 0.3, 0.4), fade), 1.0);
        }
    "#;

    let compiler = Compiler::new().expect("create bento compiler");
    let vert_result = compiler
        .compile(
            vert_source.as_bytes(),
            &Request {
                name: Some("usage_example_vert".to_string()),
                lang: ShaderLang::Glsl,
                stage: dashi::ShaderType::Vertex,
                optimization: OptimizationLevel::None,
                debug_symbols: true,
                ..Default::default()
            },
        )
        .expect("compile vertex shader");

    let frag_result = compiler
        .compile(
            frag_source.as_bytes(),
            &Request {
                name: Some("usage_example_frag".to_string()),
                lang: ShaderLang::Glsl,
                stage: dashi::ShaderType::Fragment,
                optimization: OptimizationLevel::None,
                debug_symbols: true,
                ..Default::default()
            },
        )
        .expect("compile fragment shader");

    for v in &vert_result.variables {
        println!("{} name", v.name);
    }
    let mut state = DefaultState::new(&mut ctx);
    let shaders = vec![vert_result, frag_result];

    let vert_resolver = Resolver::new(&state, &shaders[0]).expect("Unable to create resolver");
    let frag_resolver = Resolver::new(&state, &shaders[1]).expect("Unable to create resolver");

    println!(
        "Validated reserved binding in vertex shader: {:?}",
        vert_resolver.resolved()
    );
    println!(
        "Validated reserved binding in fragment shader: {:?}",
        frag_resolver.resolved()
    );

    // Build bind table layouts and bind tables from the recipe book using the reflected
    // reserved bindings. This keeps the example aligned with the way furikake consumes
    // reservations in real applications.
    let book = RecipeBook::new(&mut ctx, &state, shaders.as_slice())
        .expect("build recipe book from shaders");
    let mut bt_recipes = book.recipes();

    let mut bt_layouts: [Option<Handle<BindTableLayout>>; 4] = [None, None, None, None];
    let mut bind_tables: [Option<Handle<BindTable>>; 4] = [None, None, None, None];

    for mut recipe in bt_recipes.drain(..) {
        let set = recipe
            .bindings
            .first()
            .map(|b| b.var.set as usize)
            .unwrap_or(0);

        if set < bt_layouts.len() {
            bt_layouts[set] = Some(recipe.layout);
        }

        let bind_table = recipe.cook(&mut ctx).expect("cook bind table from recipe");

        if set < bind_tables.len() {
            bind_tables[set] = Some(bind_table);
        }
    }

    // Create vertex/index buffers for a fullscreen-ish quad.
    let vertices = vec![
        Vertex {
            position: [-0.8, -0.8],
            color: [1.0, 0.4, 0.3],
        },
        Vertex {
            position: [0.8, -0.8],
            color: [0.2, 0.8, 0.4],
        },
        Vertex {
            position: [0.8, 0.8],
            color: [0.1, 0.3, 1.0],
        },
        Vertex {
            position: [-0.8, 0.8],
            color: [0.9, 0.8, 0.2],
        },
    ];
    let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

    let vertex_buffer = ctx
        .make_buffer(&BufferInfo {
            debug_name: "usage_vertices",
            byte_size: (std::mem::size_of::<Vertex>() * vertices.len()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::VERTEX,
            initial_data: unsafe { Some(vertices.align_to::<u8>().1) },
        })
        .expect("create vertex buffer");

    let index_buffer = ctx
        .make_buffer(&BufferInfo {
            debug_name: "usage_indices",
            byte_size: (std::mem::size_of::<u32>() * indices.len()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::INDEX,
            initial_data: unsafe { Some(indices.align_to::<u8>().1) },
        })
        .expect("create index buffer");

    // Use the reserved timing bind table produced by the recipe book.
    let timing_bind_table = bind_tables[0].expect("timing bind table from recipe book");

    // Pipeline setup.
    let vert_inputs = &shaders[0].metadata.inputs;
    let position_location = vert_inputs
        .iter()
        .find(|var| var.name == "in_pos")
        .and_then(|var| var.location)
        .expect("vertex position location from metadata") as usize;
    let color_location = vert_inputs
        .iter()
        .find(|var| var.name == "in_color")
        .and_then(|var| var.location)
        .expect("vertex color location from metadata") as usize;

    let pipeline_layout = ctx
        .make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
            vertex_info: VertexDescriptionInfo {
                entries: &[
                    VertexEntryInfo {
                        format: ShaderPrimitiveType::Vec2,
                        location: position_location,
                        offset: 0,
                    },
                    VertexEntryInfo {
                        format: ShaderPrimitiveType::Vec3,
                        location: color_location,
                        offset: 8,
                    },
                ],
                stride: std::mem::size_of::<Vertex>(),
                rate: VertexRate::Vertex,
            },
            bg_layouts: [None, None, None, None],
            bt_layouts,
            shaders: &[
                PipelineShaderInfo {
                    stage: ShaderType::Vertex,
                    spirv: shaders[0].spirv.as_slice(),
                    specialization: &[],
                },
                PipelineShaderInfo {
                    stage: ShaderType::Fragment,
                    spirv: shaders[1].spirv.as_slice(),
                    specialization: &[],
                },
            ],
            details: Default::default(),
            debug_name: "usage_pipeline_layout",
        })
        .expect("create pipeline layout");

    let render_pass = ctx
        .make_render_pass(&RenderPassInfo {
            viewport: Viewport {
                area: FRect2D {
                    w: 640.0,
                    h: 480.0,
                    ..Default::default()
                },
                scissor: Rect2D {
                    w: 640,
                    h: 480,
                    ..Default::default()
                },
                ..Default::default()
            },
            subpasses: &[SubpassDescription {
                color_attachments: &[AttachmentDescription::default()],
                depth_stencil_attachment: None,
                subpass_dependencies: &[],
            }],
            debug_name: "usage_render_pass",
        })
        .expect("create render pass");

    let subpass_info = ctx
        .render_pass_subpass_info(render_pass, 0)
        .expect("render pass subpass info");
    let pipeline = ctx
        .make_graphics_pipeline(&GraphicsPipelineInfo {
            layout: pipeline_layout,
            attachment_formats: subpass_info.color_formats,
            depth_format: subpass_info.depth_format,
            subpass_samples: subpass_info.samples,
            debug_name: "usage_pipeline",
            ..Default::default()
        })
        .expect("create graphics pipeline");

    // Target to draw into.
    let target_image = ctx
        .make_image(&ImageInfo {
            debug_name: "usage_target",
            dim: [640, 480, 1],
            format: Format::RGBA8,
            initial_data: None,
            ..Default::default()
        })
        .expect("create target image");

    let target_view = ImageView {
        img: target_image,
        ..Default::default()
    };

    // Write the timing uniform and issue a single draw call.
    state.update().expect("update reserved timing");

    let mut ring = ctx
        .make_command_ring(&CommandQueueInfo2 {
            debug_name: "usage_ring",
            ..Default::default()
        })
        .expect("create command ring");

    ring.record(|list| {
        let stream = CommandStream::new()
            .begin()
            .begin_drawing(&BeginDrawing {
                viewport: Viewport {
                    area: FRect2D {
                        w: 640.0,
                        h: 480.0,
                        ..Default::default()
                    },
                    scissor: Rect2D {
                        w: 640,
                        h: 480,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                pipeline,
                color_attachments: [Some(target_view), None, None, None, None, None, None, None],
                depth_attachment: None,
                clear_values: [
                    Some(ClearValue::Color([0.05, 0.05, 0.1, 1.0])),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
                ..Default::default()
            })
            .draw_indexed(&DrawIndexed {
                vertices: vertex_buffer,
                indices: index_buffer,
                index_count: indices.len() as u32,
                bind_groups: [None, None, None, None],
                bind_tables: [Some(timing_bind_table), None, None, None],
                ..Default::default()
            })
            .stop_drawing();

        stream.end().append(list);
    })
    .expect("record draw commands");

    ring.submit(&SubmitInfo::default())
        .expect("submit draw commands");
    ring.wait_all().expect("wait for GPU work");

    println!("Rendered a quad with reserved timing binding!");
}
