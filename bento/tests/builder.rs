use bento::{
    CompilationResult, Compiler, OptimizationLevel, Request, ShaderLang,
    builder::{ComputePipelineBuilder, GraphicsPipelineBuilder, PipelineBuildError},
};
use dashi::gpu::vulkan::{Context, ContextInfo, GPUError};
use dashi::{
    BufferInfo, BufferUsage, BufferView, DebugMessageSeverity, DebugMessageType, DebugMessenger,
    DebugMessengerCreateInfo, IndexedResource, MemoryVisibility, ShaderResource,
};
use serial_test::serial;
use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const SIMPLE_COMPUTE: &str = r#"
#version 450
layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
void main() {}
"#;

const BUFFERED_COMPUTE: &str = r#"
#version 450
layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
layout(set = 0, binding = 0) uniform Config {
    uint value;
} config;
layout(set = 1, binding = 0) buffer Data {
    uint values[];
} data;
void main() {
    if (config.value == 0) {
        data.values[0] = 42;
    }
}
"#;

const GRAPHICS_VERTEX_SIMPLE: &str = r#"
#version 450
void main() {
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}
"#;

const GRAPHICS_FRAGMENT_SIMPLE: &str = r#"
#version 450
layout(location = 0) out vec4 color;
void main() {
    color = vec4(1.0, 0.0, 0.0, 1.0);
}
"#;

const GRAPHICS_VERTEX_UNIFORM: &str = r#"
#version 450
layout(set = 0, binding = 0) uniform Globals {
    vec4 position;
} globals;
void main() {
    gl_Position = globals.position;
}
"#;

const GRAPHICS_FRAGMENT_UNIFORM: &str = r#"
#version 450
layout(set = 0, binding = 0) uniform Globals {
    vec4 tint;
} globals;
layout(location = 0) out vec4 color;
void main() {
    color = globals.tint;
}
"#;

const GRAPHICS_VERTEX_SET0: &str = r#"
#version 450
layout(set = 0, binding = 0) uniform Globals {
    vec4 position;
} globals;
void main() {
    gl_Position = globals.position;
}
"#;

const GRAPHICS_FRAGMENT_SET2: &str = r#"
#version 450
layout(set = 2, binding = 0) readonly buffer Data {
    float value;
} data;
layout(location = 0) out vec4 color;
void main() {
    color = vec4(data.value, 0.0, 0.0, 1.0);
}
"#;

const COMPUTE_TABLE_SINGLE: &str = r#"
#version 450
layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
layout(set = 0, binding = 0) buffer Data {
    uint value;
} data;
void main() {
    data.value = 1;
}
"#;

const GRAPHICS_FRAGMENT_STORAGE: &str = r#"
#version 450
layout(set = 0, binding = 0) readonly buffer Data {
    float value;
} data;
layout(location = 0) out vec4 color;
void main() {
    color = vec4(data.value, 0.0, 0.0, 1.0);
}
"#;

struct ValidationContext {
    ctx: Option<Context>,
    guard: Option<ValidationGuard>,
}

impl ValidationContext {
    fn headless(info: &ContextInfo) -> Result<Self, GPUError> {
        let original_validation = std::env::var("DASHI_VALIDATION").ok();
        unsafe {
            std::env::set_var("DASHI_VALIDATION", "1");
        }

        let ctx = match Context::headless(info) {
            Ok(ctx) => ctx,
            Err(err) => {
                if let Some(value) = &original_validation {
                    unsafe {
                        std::env::set_var("DASHI_VALIDATION", value);
                    }
                } else {
                    unsafe {
                        std::env::remove_var("DASHI_VALIDATION");
                    }
                }
                return Err(err);
            }
        };

        let guard = ValidationGuard::new(&ctx, original_validation)?;

        Ok(Self {
            ctx: Some(ctx),
            guard: Some(guard),
        })
    }
}

impl std::ops::Deref for ValidationContext {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.ctx.as_ref().expect("context should exist")
    }
}

impl std::ops::DerefMut for ValidationContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx.as_mut().expect("context should exist")
    }
}

impl Drop for ValidationContext {
    fn drop(&mut self) {
        if let Some(ctx) = self.ctx.take() {
            if let Some(mut guard) = self.guard.take() {
                guard.teardown(&ctx);
            }

            ctx.destroy();
        }
    }
}

struct ValidationGuard {
    original_validation: Option<String>,
    validation_flag: Arc<AtomicBool>,
    validation_ptr: Option<*const AtomicBool>,
    debug_messenger: Option<DebugMessenger>,
}

impl ValidationGuard {
    fn new(ctx: &Context, original_validation: Option<String>) -> Result<Self, GPUError> {
        let validation_flag = Arc::new(AtomicBool::new(false));
        let validation_ptr = Arc::into_raw(Arc::clone(&validation_flag));

        let messenger_info = DebugMessengerCreateInfo {
            message_severity: DebugMessageSeverity::ERROR,
            message_type: DebugMessageType::VALIDATION,
            user_callback: validation_error_callback,
            user_data: validation_ptr as *mut c_void,
        };

        let debug_messenger = ctx.create_debug_messenger(&messenger_info)?;

        Ok(Self {
            original_validation,
            validation_flag,
            validation_ptr: Some(validation_ptr),
            debug_messenger: Some(debug_messenger),
        })
    }

    fn teardown(&mut self, ctx: &Context) {
        if let Some(messenger) = self.debug_messenger.take() {
            ctx.destroy_debug_messenger(messenger);
        }

        if let Some(ptr) = self.validation_ptr.take() {
            unsafe {
                let _ = Arc::from_raw(ptr);
            }
        }

        if let Some(value) = &self.original_validation {
            unsafe {
                std::env::set_var("DASHI_VALIDATION", value);
            }
        } else {
            unsafe {
                std::env::remove_var("DASHI_VALIDATION");
            }
        }

        assert!(
            !self.validation_flag.load(Ordering::SeqCst),
            "Vulkan validation layers reported an API usage error"
        );
    }
}

unsafe extern "system" fn validation_error_callback(
    message_severity: DebugMessageSeverity,
    message_type: DebugMessageType,
    _p_callback_data: &CStr,
    user_data: *mut c_void,
) -> bool {
    if message_severity.contains(DebugMessageSeverity::ERROR)
        && message_type.contains(DebugMessageType::VALIDATION)
    {
        if let Some(flag) =
            (!user_data.is_null()).then(|| unsafe { &*(user_data as *const AtomicBool) })
        {
            flag.store(true, Ordering::SeqCst);
        }
    }

    false
}

fn compile_shader(stage: dashi::ShaderType, source: &str) -> CompilationResult {
    let compiler = Compiler::new().expect("compiler should initialize");
    let request = Request {
        name: None,
        lang: ShaderLang::Glsl,
        stage,
        optimization: OptimizationLevel::Performance,
        debug_symbols: false,
        defines: HashMap::new(),
    };

    compiler
        .compile(source.as_bytes(), &request)
        .expect("shader should compile")
}

#[test]
#[serial]
fn builds_simple_compute_pipeline_without_validation_errors() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, SIMPLE_COMPUTE);

    let pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .build(&mut ctx);

    assert!(pipeline.is_ok());
}

#[test]
#[serial]
fn builds_compute_pipeline_with_resources_and_table_updates() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, BUFFERED_COMPUTE);

    let uniform = ctx
        .make_buffer(&BufferInfo {
            debug_name: "config",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("uniform buffer");

    let mut pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .add_variable("config", ShaderResource::Buffer(uniform.into()))
        .add_table_variable("data", 2)
        .build(&mut ctx)
        .expect("pipeline should build");

    let replacement = ctx
        .make_buffer(&BufferInfo {
            debug_name: "replacement",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("replacement buffer");

    pipeline.update_table(
        "data",
        IndexedResource {
            resource: ShaderResource::StorageBuffer(replacement.into()),
            slot: 1,
        },
    );

    let replacement_second = ctx
        .make_buffer(&BufferInfo {
            debug_name: "replacement_second",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("second replacement buffer");

    pipeline.update_table_slice(
        "data",
        &[IndexedResource {
            resource: ShaderResource::StorageBuffer(replacement_second.into()),
            slot: 0,
        }],
    );
}

#[test]
#[serial]
fn compute_pipeline_fails_when_data_is_missing() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, BUFFERED_COMPUTE);

    let uniform = ctx
        .make_buffer(&BufferInfo {
            debug_name: "config_missing_data",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("uniform buffer");

    let pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .add_variable("config", ShaderResource::Buffer(uniform.into()))
        .build(&mut ctx);

    match pipeline {
        Err(PipelineBuildError::MissingBindings { bindings }) => {
            assert!(bindings.iter().any(|binding| binding.name == "data"));
        }
        other => panic!("expected missing bindings error, got {other:?}"),
    }
}

#[test]
#[serial]
fn builds_compute_pipeline_with_initial_table_resources() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, BUFFERED_COMPUTE);

    let uniform = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "config",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("uniform buffer"),
    );

    let first_storage = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "table_entry_0",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("first storage buffer"),
    );

    let second_storage = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "table_entry_1",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("second storage buffer"),
    );

    let initial_resources = vec![
        IndexedResource {
            resource: ShaderResource::StorageBuffer(first_storage),
            slot: 0,
        },
        IndexedResource {
            resource: ShaderResource::StorageBuffer(second_storage),
            slot: 1,
        },
    ];

    let replacement = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "replacement_initial_table",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("replacement buffer"),
    );

    let mut pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .add_variable("config", ShaderResource::Buffer(uniform))
        .add_table_variable_with_resources("data", initial_resources)
        .build(&mut ctx)
        .expect("pipeline should build with initial resources");

    pipeline.update_table_slice(
        "data",
        &[IndexedResource {
            resource: ShaderResource::StorageBuffer(replacement),
            slot: 1,
        }],
    );
}

#[test]
#[serial]
fn compute_table_count_can_be_overridden_with_resources_length() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, COMPUTE_TABLE_SINGLE);
    let data_name = compute_stage
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 0)
        .map(|var| var.name.clone())
        .expect("data variable name");

    let first = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "override_first",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("first override buffer"),
    );
    let second = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "override_second",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("second override buffer"),
    );

    let pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .add_table_variable_with_resources(
            &data_name,
            vec![
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(first),
                    slot: 0,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(second),
                    slot: 1,
                },
            ],
        )
        .build(&mut ctx);

    assert!(pipeline.is_ok());
}

#[test]
#[serial]
fn compute_table_rejects_out_of_range_slots() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, COMPUTE_TABLE_SINGLE);
    let data_name = compute_stage
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 0)
        .map(|var| var.name.clone())
        .expect("data variable name");

    let invalid = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "invalid_slot",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("invalid buffer"),
    );

    let pipeline = ComputePipelineBuilder::new()
        .shader_compiled(Some(compute_stage))
        .add_table_variable_with_resources(
            &data_name,
            vec![IndexedResource {
                resource: ShaderResource::StorageBuffer(invalid),
                slot: 2,
            }],
        )
        .build(&mut ctx);
    assert!(pipeline.is_err());
}

#[test]
#[serial]
fn builds_graphics_pipeline_without_resources() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_SIMPLE);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_SIMPLE);

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .build(&mut ctx);

    assert!(pipeline.is_ok());
}

#[test]
#[serial]
fn builds_graphics_pipeline_with_shared_uniform_bindings() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_UNIFORM);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_UNIFORM);

    let globals = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "globals",
            byte_size: 64,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("globals buffer"),
    );

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .add_variable("globals", ShaderResource::Buffer(globals))
        .build(&mut ctx).expect("Failed to make pipeline");
}

#[test]
#[serial]
fn graphics_pipeline_fails_when_data_is_missing() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_UNIFORM);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_UNIFORM);

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .build(&mut ctx);

    match pipeline {
        Err(PipelineBuildError::MissingBindings { bindings }) => {
            assert!(bindings.iter().any(|binding| binding.name == "globals"));
        }
        other => panic!("expected missing bindings error, got {other:?}"),
    }
}

#[test]
#[serial]
fn graphics_pipeline_tracks_multiple_sets() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_SET0);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_SET2);

    let globals_name = vertex
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 0)
        .map(|var| var.name.clone())
        .expect("globals variable name");
    let data_name = fragment
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 2)
        .map(|var| var.name.clone())
        .expect("data variable name");

    let globals = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_globals",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("globals buffer"),
    );

    let data = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_data",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("data buffer"),
    );

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .add_variable(&globals_name, ShaderResource::Buffer(globals))
        .add_variable(&data_name, ShaderResource::StorageBuffer(data))
        .build(&mut ctx);

    assert!(pipeline.is_ok());

    let pipeline = pipeline.expect("pipeline");
    let tables = pipeline.tables();
    assert!(tables[0].is_some());
    assert!(tables[2].is_some());
    assert!(tables[1].is_none());
}

#[test]
#[serial]
fn graphics_table_count_can_be_overridden() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_SIMPLE);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_STORAGE);
    let data_name = fragment
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 0)
        .map(|var| var.name.clone())
        .expect("data variable name");

    let first = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_override_first",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("graphics override buffer"),
    );
    let second = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_override_second",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("graphics second override buffer"),
    );
    let third = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_override_third",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("graphics third override buffer"),
    );

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .add_table_variable_with_resources(
            &data_name,
            vec![
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(first),
                    slot: 0,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(second),
                    slot: 1,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(third),
                    slot: 2,
                },
            ],
        )
        .build(&mut ctx);

    assert!(pipeline.is_ok());
}

#[test]
#[serial]
fn test_cull_shader_binding_mix() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, BUFFERED_COMPUTE);

    let storage = ctx
        .make_buffer(&BufferInfo {
            debug_name: "config",
            byte_size: 256,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("uniform buffer");

    let uniform = ctx
        .make_buffer(&BufferInfo {
            debug_name: "config",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("uniform buffer");

    let mut cso = ComputePipelineBuilder::new()
        .shader(Some(
            include_str!("fixtures/scene_cull.comp.glsl").as_bytes(),
        ))
        .add_table_variable_with_resources(
            "cameras",
            vec![
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(storage.into()),
                    slot: 0,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(storage.into()),
                    slot: 1,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(storage.into()),
                    slot: 2,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(storage.into()),
                    slot: 3,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(storage.into()),
                    slot: 4,
                },
            ],
        )
        .add_variable("objects", ShaderResource::StorageBuffer(storage.into()))
        .add_variable("bins", ShaderResource::StorageBuffer(storage.into()))
        .add_variable("culled", ShaderResource::StorageBuffer(storage.into()))
        .add_variable("counts", ShaderResource::StorageBuffer(storage.into()))
        .add_variable("camera", ShaderResource::ConstBuffer(uniform.into()))
        .add_variable("params", ShaderResource::ConstBuffer(uniform.into()))
        .build(&mut ctx);

    assert!(cso.is_ok());

    let cso = cso.unwrap();

    assert!(cso.tables()[0].is_some());
    assert!(cso.tables()[1].is_some());
}

#[test]
#[serial]
fn graphics_table_rejects_out_of_range_slots() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_SIMPLE);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_STORAGE);
    let data_name = fragment
        .variables
        .iter()
        .find(|var| var.kind.binding == 0 && var.set == 0)
        .map(|var| var.name.clone())
        .expect("data variable name");

    let invalid = BufferView::new(
        ctx.make_buffer(&BufferInfo {
            debug_name: "graphics_invalid_slot",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("graphics invalid buffer"),
    );

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .add_table_variable_with_resources(
            &data_name,
            vec![IndexedResource {
                resource: ShaderResource::StorageBuffer(invalid),
                slot: 4,
            }],
        )
        .build(&mut ctx);
    assert!(pipeline.is_err());
}
