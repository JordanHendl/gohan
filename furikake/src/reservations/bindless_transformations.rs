#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, BufferView, CommandStream, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::Transformation};

use super::{DirtyRange, ReservedBinding, ReservedItem, table_binding_from_indexed};

pub struct ReservedBindlessTransformations {
    ctx: NonNull<Context>,
    data: StagedBuffer,
    available: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessTransformations {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 8162;

        let available: Vec<u16> = (0..START_SIZE as u16).collect();
        let data = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Transformation Buffer",
                byte_size: std::mem::size_of::<Transformation>() as u32 * START_SIZE as u32,
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

    pub fn remove_transform(&mut self, transform: Handle<Transformation>) {
        if transform.valid() && (transform.slot as usize) < 512 {
            self.available.push(transform.slot);
        }
    }

    pub fn add_transform(&mut self) -> Handle<Transformation> {
        if let Some(id) = self.available.pop() {
            return Handle::new(id, 0);
        }

        return Handle::new(u16::MAX, u16::MAX);
    }

    pub fn push_transform(&mut self, transform: Transformation) -> Handle<Transformation> {
        let handle = self.add_transform();
        if handle.valid() {
            *self.transform_mut(handle) = transform;
        }
        handle
    }

    pub fn transform(&self, handle: Handle<Transformation>) -> &Transformation {
        &self.data.as_slice()[handle.slot as usize]
    }

    pub fn transform_mut(&mut self, handle: Handle<Transformation>) -> &mut Transformation {
        self.dirty
            .mark_elements::<Transformation>(handle.slot as usize, 1);
        &mut self.data.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessTransformations {
    fn name(&self) -> String {
        "meshi_bindless_transformations".to_string()
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
