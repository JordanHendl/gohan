#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::SkeletonHeader};

use super::{table_binding_from_indexed, ReservedBinding, ReservedItem};

pub struct ReservedBindlessSkeletons {
    ctx: NonNull<Context>,
    skeletons: StagedBuffer,
    available_skeletons: Vec<u16>,
}

impl ReservedBindlessSkeletons {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SKELETONS: usize = 512;

        let available_skeletons: Vec<u16> = (0..START_SKELETONS as u16).collect();
        let skeletons = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Skeleton Header Buffer",
                byte_size: std::mem::size_of::<SkeletonHeader>() as u32 * START_SKELETONS as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            skeletons,
            available_skeletons,
        }
    }

    pub fn add_skeleton(&mut self) -> Handle<SkeletonHeader> {
        self.available_skeletons
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn remove_skeleton(&mut self, skeleton: Handle<SkeletonHeader>) {
        if skeleton.valid() && (skeleton.slot as usize) < self.skeletons.as_slice::<SkeletonHeader>().len() {
            self.available_skeletons.push(skeleton.slot);
        }
    }

    pub fn skeleton(&self, handle: Handle<SkeletonHeader>) -> &SkeletonHeader {
        &self.skeletons.as_slice()[handle.slot as usize]
    }

    pub fn skeleton_mut(&mut self, handle: Handle<SkeletonHeader>) -> &mut SkeletonHeader {
        &mut self.skeletons.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessSkeletons {
    fn name(&self) -> String {
        "meshi_bindless_skeletons".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(self.skeletons.sync_up().end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.skeletons.device().into()),
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
