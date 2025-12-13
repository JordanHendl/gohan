use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ash::vk;
use bento::{
    CompilationResult, Compiler, OptimizationLevel, Request, ShaderLang,
    builder::{ComputePipelineBuilder, GraphicsPipelineBuilder},
};
use dashi::gpu::vulkan::{Context, ContextInfo, GPUError};
use dashi::{BufferInfo, BufferUsage, BufferView, IndexedResource, MemoryVisibility, ShaderResource};
use serial_test::serial;

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
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
}

impl ValidationGuard {
    fn new(ctx: &Context, original_validation: Option<String>) -> Result<Self, GPUError> {
        let validation_flag = Arc::new(AtomicBool::new(false));
        let validation_ptr = Arc::into_raw(Arc::clone(&validation_flag));

        let messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION)
            .pfn_user_callback(Some(validation_error_callback))
            .user_data(validation_ptr as *mut c_void);

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
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    _p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    user_data: *mut c_void,
) -> vk::Bool32 {
    if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
        && message_type.contains(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION)
    {
        if let Some(flag) =
            (!user_data.is_null()).then(|| unsafe { &*(user_data as *const AtomicBool) })
        {
            flag.store(true, Ordering::SeqCst);
        }
    }

    vk::FALSE
}

fn compile_shader(stage: dashi::ShaderType, source: &str) -> CompilationResult {
    let compiler = Compiler::new().expect("compiler should initialize");
    let request = Request {
        name: None,
        lang: ShaderLang::Glsl,
        stage,
        optimization: OptimizationLevel::Performance,
        debug_symbols: false,
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

    assert!(pipeline.is_some());
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
fn builds_compute_pipeline_with_initial_table_resources() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");
    let compute_stage = compile_shader(dashi::ShaderType::Compute, BUFFERED_COMPUTE);

    let uniform = BufferView::new(ctx
        .make_buffer(&BufferInfo {
            debug_name: "config",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::UNIFORM,
            initial_data: None,
        })
        .expect("uniform buffer"));

    let first_storage = BufferView::new(ctx
        .make_buffer(&BufferInfo {
            debug_name: "table_entry_0",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("first storage buffer"));

    let second_storage = BufferView::new(ctx
        .make_buffer(&BufferInfo {
            debug_name: "table_entry_1",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("second storage buffer"));

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

    let replacement = BufferView::new(ctx
        .make_buffer(&BufferInfo {
            debug_name: "replacement_initial_table",
            byte_size: 16,
            visibility: MemoryVisibility::CpuAndGpu,
            usage: BufferUsage::STORAGE,
            initial_data: None,
        })
        .expect("replacement buffer"));

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
fn builds_graphics_pipeline_without_resources() {
    let mut ctx = ValidationContext::headless(&ContextInfo::default()).expect("headless context");

    let vertex = compile_shader(dashi::ShaderType::Vertex, GRAPHICS_VERTEX_SIMPLE);
    let fragment = compile_shader(dashi::ShaderType::Fragment, GRAPHICS_FRAGMENT_SIMPLE);

    let pipeline = GraphicsPipelineBuilder::new()
        .vertex_compiled(Some(vertex))
        .fragment_compiled(Some(fragment))
        .build(&mut ctx);

    assert!(pipeline.is_some());
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
        .add_variable("Globals", ShaderResource::Buffer(globals))
        .build(&mut ctx);

    assert!(pipeline.is_some());
}
