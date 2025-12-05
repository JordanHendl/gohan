use std::ptr::NonNull;

use dashi::{
    BindGroup, BindTable, ComputePipeline, ComputePipelineLayout, Context, GraphicsPipeline, GraphicsPipelineDetails, GraphicsPipelineLayout, Handle, IndexedResource, ShaderResource
};

use crate::CompilationResult;

pub struct PSO {
    pub layout: Handle<GraphicsPipelineLayout>,
    pub handle: Handle<GraphicsPipeline>,
    pub bind_groups: Vec<Handle<BindGroup>>,
    pub bind_table: Vec<Handle<BindTable>>,
    ctx: NonNull<Context>,
}

impl PSO {
    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        todo!()
    }
}

pub struct GraphicsPipelineBuilder {}

impl GraphicsPipelineBuilder {
    pub fn new() -> Self {
        todo!()
    }

    pub fn vertex(self, shader: Option<&[u8]>) -> Self {
        todo!()
    }

    pub fn vertex_compiled(self, shader: Option<CompilationResult>) -> Self {
        todo!()
    }

    pub fn fragment(self, shader: Option<&[u8]>) -> Self {
        todo!()
    }

    pub fn fragment_compiled(self, shader: Option<CompilationResult>) -> Self {
        todo!()
    }

    // Adds a variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, variable: IndexedResource) -> Self {
        todo!()
    }

    pub fn add_variable(self, key: &str, variable: ShaderResource) -> Self {
        todo!()
    }

    pub fn set_details(self, details: GraphicsPipelineDetails) -> Self {
        todo!()
    }

    pub fn build(self, ctx: &mut dashi::Context) -> Option<PSO> {
        todo!()
    }
}

////////////////////////////////////////////////////////////////////////////
///

pub struct CSO {
    layout: Handle<ComputePipelineLayout>,
    handle: Handle<ComputePipeline>,
    bind_groups: Vec<Handle<BindGroup>>,
    bind_table: Vec<Handle<BindTable>>,
}

impl CSO {
    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        todo!()
    }
}
pub struct ComputePipelineBuilder {}

impl ComputePipelineBuilder {
    pub fn new() -> Self {
        todo!()
    }

    pub fn shader(self, shader: Option<&[u8]>) -> Self {
        todo!()
    }

    pub fn shader_compiled(self, shader: Option<CompilationResult>) -> Self {
        todo!()
    }

    // Adds a bind table variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, variable: IndexedResource) -> Self {
        todo!()
    }

    // Adds a variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_variable(self, key: &str, variable: ShaderResource) -> Self {
        todo!()
    }

    // Will fail if shaders are not given, or if variables given do not 
    pub fn build(self, ctx: &mut dashi::Context) -> Option<CSO> {
        todo!()
    }
}
