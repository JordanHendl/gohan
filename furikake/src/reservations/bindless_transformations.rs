#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{BufferInfo, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource};

use crate::types::Transformation;

use super::{ReservedBinding, ReservedItem};

pub struct ReservedBindlessTransformations {
    ctx: NonNull<Context>,
    device_transformation_data: Vec<IndexedResource>,
    host_transformation_data: Vec<NonNull<Transformation>>,
    available: Vec<u16>,
}

impl ReservedBindlessTransformations {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 512;

        let mut d_data = Vec::with_capacity(START_SIZE);
        let mut h_data = Vec::with_capacity(START_SIZE);
        let available: Vec<u16> = (0..START_SIZE as u16).collect();

        for i in 0..START_SIZE {
            let default = [Transformation::default()];
            let buf = ctx
                .make_buffer(&BufferInfo {
                    debug_name: &format!("[FURIKAKE] Bindless Transformation {}", i),
                    byte_size: std::mem::size_of::<Transformation>() as u32,
                    visibility: dashi::MemoryVisibility::CpuAndGpu,
                    usage: dashi::BufferUsage::STORAGE,
                    initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                })
                .expect("Failed making transformation buffer");

            let h = ctx
                .map_buffer_mut::<Transformation>(buf)
                .expect("Failed to map buffer");
            let nnt =
                NonNull::new(h.as_mut_ptr()).expect("NonNull failed check for transformation map!");

            h_data.push(nnt);
            d_data.push(IndexedResource {
                resource: ShaderResource::StorageBuffer(buf),
                slot: i as u32,
            });
        }

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            device_transformation_data: d_data,
            host_transformation_data: h_data,
            available,
        }
    }

    pub fn extend(&mut self) {
        let ctx: &mut Context = unsafe { self.ctx.as_mut() };
        if self.available.is_empty() {
            const EXTENSION_SIZE: usize = 128;
            let start = self.host_transformation_data.len();
            let end = start + EXTENSION_SIZE;
            for i in start..end {
                let default = [Transformation::default()];
                let buf = ctx
                    .make_buffer(&BufferInfo {
                        debug_name: &format!("[FURIKAKE] Bindless Transformation {}", i),
                        byte_size: std::mem::size_of::<Transformation>() as u32,
                        visibility: dashi::MemoryVisibility::CpuAndGpu,
                        usage: dashi::BufferUsage::STORAGE,
                        initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                    })
                    .expect("Failed making transformation buffer");

                let h = ctx
                    .map_buffer_mut::<Transformation>(buf)
                    .expect("Failed to map buffer");
                let nnt = NonNull::new(h.as_mut_ptr())
                    .expect("NonNull failed check for transformation map!");

                self.host_transformation_data.push(nnt);
                self.device_transformation_data.push(IndexedResource {
                    resource: ShaderResource::StorageBuffer(buf),
                    slot: i as u32,
                });
            }
        }
    }

    pub fn remove_transformation(&mut self, transformation: Handle<Transformation>) {
        if transformation.valid()
            && (transformation.slot as usize) < self.device_transformation_data.len()
        {
            self.available.push(transformation.slot);
        }
    }

    pub fn add_transformation(&mut self) -> Handle<Transformation> {
        if let Some(id) = self.available.pop() {
            Handle::new(id, 0)
        } else {
            self.extend();
            self.add_transformation()
        }
    }

    pub fn transformation(&self, handle: Handle<Transformation>) -> &Transformation {
        unsafe { self.host_transformation_data[handle.slot as usize].as_ref() }
    }

    pub fn transformation_mut(&mut self, handle: Handle<Transformation>) -> &mut Transformation {
        unsafe { self.host_transformation_data[handle.slot as usize].as_mut() }
    }
}

impl ReservedItem for ReservedBindlessTransformations {
    fn name(&self) -> String {
        "meshi_bindless_transformations".to_string()
    }

    fn update(&mut self, _ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        ReservedBinding::BindlessBinding(IndexedBindingInfo {
            resources: &self.device_transformation_data,
            binding: 0,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashi::{Context, ContextInfo};
    use glam::Mat4;

    #[test]
    fn reuses_transformation_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut transformations = ReservedBindlessTransformations::new(&mut ctx);

        let first = transformations.add_transformation();
        let second = transformations.add_transformation();
        assert_ne!(first.slot, second.slot);

        transformations.remove_transformation(first);
        let reused = transformations.add_transformation();

        assert_eq!(first.slot, reused.slot);
    }

    #[test]
    fn writes_transformation_data() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut transformations = ReservedBindlessTransformations::new(&mut ctx);

        let handle = transformations.add_transformation();
        {
            let transform = transformations.transformation_mut(handle);
            transform.transform = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        }

        transformations
            .update(&mut ctx)
            .expect("update transformations");

        let transform = transformations.transformation(handle);
        assert_eq!(
            transform.transform,
            Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
        );
    }
}
