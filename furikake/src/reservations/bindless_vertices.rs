#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::VertexBufferSlot};

use super::{table_binding_from_indexed, ReservedBinding, ReservedItem};

const VERTEX_BUFFER_BYTES: u32 = 16 * 1024 * 1024;
const VERTEX_BUFFER_SLOT_COUNT: usize = crate::types::VERTEX_BUFFER_SLOT_COUNT;

const VERTEX_BUFFER_NAMES: [&str; VERTEX_BUFFER_SLOT_COUNT] = [
    "[FURIKAKE] Skeleton Vertex Buffer",
    "[FURIKAKE] Simple Vertex Buffer",
];

pub struct ReservedBindlessVertices {
    ctx: NonNull<Context>,
    buffers: Vec<StagedBuffer>,
    write_offsets: [u32; VERTEX_BUFFER_SLOT_COUNT],
}

impl ReservedBindlessVertices {
    pub fn new(ctx: &mut Context) -> Self {
        let buffers = VERTEX_BUFFER_NAMES
            .iter()
            .map(|name| {
                StagedBuffer::new(
                    ctx,
                    BufferInfo {
                        debug_name: name,
                        byte_size: VERTEX_BUFFER_BYTES,
                        visibility: Default::default(),
                        usage: BufferUsage::ALL,
                        initial_data: None,
                    },
                )
            })
            .collect();

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            buffers,
            write_offsets: [0; VERTEX_BUFFER_SLOT_COUNT],
        }
    }

    pub fn buffer(&self, slot: VertexBufferSlot) -> &StagedBuffer {
        &self.buffers[slot as usize]
    }

    pub fn buffer_mut(&mut self, slot: VertexBufferSlot) -> &mut StagedBuffer {
        &mut self.buffers[slot as usize]
    }

    pub fn vertices(&self, slot: VertexBufferSlot) -> &[u8] {
        self.buffer(slot).as_slice()
    }

    pub fn vertices_mut(&mut self, slot: VertexBufferSlot) -> &mut [u8] {
        self.buffer_mut(slot).as_slice_mut()
    }

    pub fn push_vertex_bytes(&mut self, slot: VertexBufferSlot, bytes: &[u8]) -> Option<u32> {
        let offset = self.write_offsets[slot.as_index()] as usize;
        let buffer = self.buffer_mut(slot).as_slice_mut::<u8>();
        let end = offset.saturating_add(bytes.len());
        if end > buffer.len() {
            return None;
        }
        buffer[offset..end].copy_from_slice(bytes);
        self.write_offsets[slot.as_index()] = end as u32;
        Some(offset as u32)
    }
}

impl ReservedItem for ReservedBindlessVertices {
    fn name(&self) -> String {
        "meshi_bindless_vertices".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        for buffer in &self.buffers {
            cmd = cmd.combine(buffer.sync_up().end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        let resources: Vec<IndexedResource> = self
            .buffers
            .iter()
            .enumerate()
            .map(|(slot, buffer)| IndexedResource {
                resource: ShaderResource::StorageBuffer(buffer.device().into()),
                slot: slot as u32,
            })
            .collect();

        table_binding_from_indexed(IndexedBindingInfo {
            resources: resources.as_slice(),
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
