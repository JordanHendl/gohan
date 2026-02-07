#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    BufferInfo, BufferUsage, BufferView, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource, cmd::Executable,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::Camera};

use super::{DirtyRange, ReservedBinding, ReservedItem, table_binding_from_indexed};

pub struct ReservedBindlessCamera {
    ctx: NonNull<Context>,
    data: StagedBuffer,
    available: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessCamera {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 2048;

        let available: Vec<u16> = (0..START_SIZE as u16).collect();
        let data = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Camera Buffer",
                byte_size: std::mem::size_of::<Camera>() as u32 * START_SIZE as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            data,
            available,
            dirty: DirtyRange::default(),
        }
    }

    pub fn remove_camera(&mut self, camera: Handle<Camera>) {
        if camera.valid() && (camera.slot as usize) < 512 {
            self.available.push(camera.slot);
        }
    }

    pub fn add_camera(&mut self) -> Handle<Camera> {
        if let Some(id) = self.available.pop() {
            return Handle::new(id, 0);
        }

        return Handle::new(0, 0);
    }

    pub fn push_camera(&mut self, camera: Camera) -> Handle<Camera> {
        let handle = self.add_camera();
        if handle.valid() {
            *self.camera_mut(handle) = camera;
        }
        handle
    }

    pub fn camera(&self, handle: Handle<Camera>) -> &Camera {
        &self.data.as_slice()[handle.slot as usize]
    }

    pub fn camera_mut(&mut self, handle: Handle<Camera>) -> &mut Camera {
        self.dirty.mark_elements::<Camera>(handle.slot as usize, 1);
        &mut self.data.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessCamera {
    fn name(&self) -> String {
        "meshi_bindless_camera".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.data.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        return table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.data.device().into()),
                slot: 0,
            }],
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

#[cfg(test)]
mod tests {
    use super::*;
    use dashi::{Context, ContextInfo};
    use glam::{Quat, Vec3};

    #[test]
    fn reuses_released_camera_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut cameras = ReservedBindlessCamera::new(&mut ctx);

        let first = cameras.add_camera();
        let second = cameras.add_camera();
        assert_ne!(first.slot, second.slot);

        cameras.remove_camera(first);
        let reused = cameras.add_camera();

        assert_eq!(first.slot, reused.slot);
    }

    #[test]
    fn mutates_host_camera_data() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut cameras = ReservedBindlessCamera::new(&mut ctx);

        let handle = cameras.add_camera();
        {
            let cam = cameras.camera_mut(handle);
            cam.set_position(Vec3::new(1.0, 2.0, 3.0));
            cam.set_rotation(Quat::from_rotation_y(1.0));
        }

        let cam = cameras.camera(handle);
        assert_eq!(cam.position(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(cam.rotation(), Quat::from_rotation_y(1.0));
    }
}
