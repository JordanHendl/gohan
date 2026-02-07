#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, BufferView, CommandStream, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::Material};

use super::{DirtyRange, ReservedBinding, ReservedItem, table_binding_from_indexed};

pub struct ReservedBindlessMaterials {
    ctx: NonNull<Context>,
    data: StagedBuffer,
    available: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessMaterials {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 4096;

        let available: Vec<u16> = (0..START_SIZE as u16).collect();
        let data = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Material Buffer",
                byte_size: std::mem::size_of::<Material>() as u32 * START_SIZE as u32,
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

    pub fn remove_material(&mut self, material: Handle<Material>) {
        if material.valid() && (material.slot as usize) < 512 {
            self.available.push(material.slot);
        }
    }

    pub fn add_material(&mut self) -> Handle<Material> {
        if let Some(id) = self.available.pop() {
            return Handle::new(id, 0);
        }

        return Handle::new(0, 0);
    }

    pub fn push_material(&mut self, material: Material) -> Handle<Material> {
        let handle = self.add_material();
        if handle.valid() {
            *self.material_mut(handle) = material;
        }
        handle
    }

    pub fn material(&self, handle: Handle<Material>) -> &Material {
        &self.data.as_slice()[handle.slot as usize]
    }

    pub fn material_mut(&mut self, handle: Handle<Material>) -> &mut Material {
        self.dirty
            .mark_elements::<Material>(handle.slot as usize, 1);
        &mut self.data.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessMaterials {
    fn name(&self) -> String {
        "meshi_bindless_materials".to_string()
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

mod test {
    use crate::types::*;

    #[test]
    fn ensure_size_of_material() {
        use crate::types::JointTransform;

        assert_eq!(std::mem::size_of::<Material>(), 32);
    }
}
