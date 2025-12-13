#![allow(dead_code)]

use dashi::{BindGroupVariable, BindingInfo, Buffer, BufferView, Context, Handle, ShaderResource};

use crate::types::Camera;

use super::{ReservedBinding, ReservedItem};

pub(crate) struct ReservedCamera {
    camera: Camera,
    buffer: BufferView,
    variable: BindGroupVariable,
}

#[repr(C)]
struct Data {
    transform: glam::Mat4,
}

impl ReservedCamera {
    pub fn new(_ctx: &mut Context) -> Self {
        todo!()
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
}

impl ReservedItem for ReservedCamera {
    fn name(&self) -> String {
        "meshi_camera".to_string()
    }

    fn update(&mut self, ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        let s = ctx
            .map_buffer_mut::<Data>(self.buffer)
            .map_err(crate::error::FurikakeError::buffer_map_failed)?;
        // update transform

        s[0].transform = self.camera.view_matrix();

        ctx.unmap_buffer(self.buffer.handle)
            .map_err(crate::error::FurikakeError::buffer_unmap_failed)?;

        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        return ReservedBinding::Binding(BindingInfo {
            resource: ShaderResource::ConstBuffer(BufferView {
                handle: self.buffer.handle,
                size: (std::mem::size_of::<Data>()) as u64,
                offset: 0,
            }),
            binding: 0,
        });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
