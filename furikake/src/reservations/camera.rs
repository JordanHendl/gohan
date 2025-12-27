#![allow(dead_code)]

use dashi::{cmd::Executable, BindTableVariable, BufferView, CommandStream, Context, IndexedResource, ShaderResource};

use crate::{error::FurikakeError, types::Camera};

use super::{ReservedBinding, ReservedItem};

pub(crate) struct ReservedCamera {
    camera: Camera,
    buffer: BufferView,
    variable: BindTableVariable,
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

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(CommandStream::new().begin().end())
    }

    fn binding(&self) -> ReservedBinding {
        ReservedBinding::TableBinding {
            binding: 0,
            resources: vec![IndexedResource {
                resource: ShaderResource::ConstBuffer(BufferView {
                    handle: self.buffer.handle,
                    size: (std::mem::size_of::<Data>()) as u64,
                    offset: 0,
                }),
                slot: 0,
            }],
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
