#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, BufferView, CommandStream, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::Light};

use super::{DirtyRange, ReservedBinding, ReservedItem, table_binding_from_indexed};

pub struct ReservedBindlessLights {
    ctx: NonNull<Context>,
    data: StagedBuffer,
    available: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessLights {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 4096;

        let available: Vec<u16> = (0..START_SIZE as u16).collect();
        let start = vec![Light::default(); START_SIZE];
        let data = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Light Buffer",
                byte_size: std::mem::size_of::<Light>() as u32 * START_SIZE as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: unsafe{Some(start.as_slice().align_to::<u8>().1)},
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            data,
            available,
            dirty: DirtyRange::default(),
        }
    }

    pub fn remove_light(&mut self, light: Handle<Light>) {
        if light.valid() && (light.slot as usize) < 512 {
            self.available.push(light.slot);
        }
    }

    pub fn add_light(&mut self) -> Handle<Light> {
        if let Some(id) = self.available.pop() {
            return Handle::new(id, 0);
        }

        return Handle::new(0, 0);
    }

    pub fn push_light(&mut self, light: Light) -> Handle<Light> {
        let handle = self.add_light();
        if handle.valid() {
            *self.light_mut(handle) = light;
        }
        handle
    }

    pub fn light(&self, handle: Handle<Light>) -> &Light {
        &self.data.as_slice()[handle.slot as usize]
    }

    pub fn light_mut(&mut self, handle: Handle<Light>) -> &mut Light {
        self.dirty.mark_elements::<Light>(handle.slot as usize, 1);
        &mut self.data.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessLights {
    fn name(&self) -> String {
        "meshi_bindless_lights".to_string()
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
