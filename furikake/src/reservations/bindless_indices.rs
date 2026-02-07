#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::error::FurikakeError;

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

const INDEX_BUFFER_BYTES: u32 = 32 * 1024 * 1024;

pub struct ReservedBindlessIndices {
    ctx: NonNull<Context>,
    indices: StagedBuffer,
    next_index: u32,
    dirty: DirtyRange,
}

impl ReservedBindlessIndices {
    pub fn new(ctx: &mut Context) -> Self {
        let indices = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Index Buffer",
                byte_size: INDEX_BUFFER_BYTES,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            indices,
            next_index: 0,
            dirty: DirtyRange::default(),
        }
    }

    pub fn indices(&self) -> &[u32] {
        self.indices.as_slice()
    }

    pub fn indices_mut(&mut self) -> &mut [u32] {
        let len = self.indices.as_slice::<u32>().len();
        self.dirty.mark_elements::<u32>(0, len);
        self.indices.as_slice_mut()
    }

    pub fn push_indices(&mut self, data: &[u32]) -> Option<u32> {
        let offset = self.next_index as usize;
        let buffer = self.indices.as_slice_mut::<u32>();
        let end = offset.saturating_add(data.len());
        if end > buffer.len() {
            return None;
        }
        buffer[offset..end].copy_from_slice(data);
        self.next_index = end as u32;
        self.dirty
            .mark_elements::<u32>(offset, data.len());
        Some(offset as u32)
    }
}

impl ReservedItem for ReservedBindlessIndices {
    fn name(&self) -> String {
        "meshi_bindless_indices".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            //cmd = cmd.combine(self.indices.sync_up_range(start, end - start).end());
            cmd = cmd.combine(self.indices.sync_up());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.indices.device().into()),
                slot: 0,
            }],
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
