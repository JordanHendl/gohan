#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::PerObjectJointTransform};

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

#[derive(Clone, Copy, Debug)]
pub struct PerObjectJointAllocation {
    pub offset: u32,
    pub count: u32,
}

pub struct ReservedPerObjJoints {
    ctx: NonNull<Context>,
    joints: StagedBuffer,
    free_ranges: Vec<PerObjectJointAllocation>,
    dirty: DirtyRange,
}

impl ReservedPerObjJoints {
    pub fn new(ctx: &mut Context) -> Self {
        const START_JOINTS: usize = 16_384;

        let joints = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Per-Object Joint Buffer",
                byte_size: std::mem::size_of::<PerObjectJointTransform>() as u32 * START_JOINTS as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            joints,
            free_ranges: vec![PerObjectJointAllocation {
                offset: 0,
                count: START_JOINTS as u32,
            }],
            dirty: DirtyRange::default(),
        }
    }

    pub fn reserve(&mut self, joint_count: u32) -> Option<PerObjectJointAllocation> {
        if joint_count == 0 {
            return None;
        }

        let (index, range) = self
            .free_ranges
            .iter()
            .enumerate()
            .find(|(_, range)| range.count >= joint_count)?;

        let allocation = PerObjectJointAllocation {
            offset: range.offset,
            count: joint_count,
        };

        if range.count == joint_count {
            self.free_ranges.remove(index);
        } else {
            self.free_ranges[index].offset += joint_count;
            self.free_ranges[index].count -= joint_count;
        }

        Some(allocation)
    }

    pub fn free(&mut self, allocation: PerObjectJointAllocation) {
        if allocation.count == 0 {
            return;
        }

        self.free_ranges.push(allocation);
        self.coalesce_free_ranges();
    }

    pub fn joints_for(
        &self,
        allocation: PerObjectJointAllocation,
    ) -> &[PerObjectJointTransform] {
        let start = allocation.offset as usize;
        let end = start + allocation.count as usize;
        &self.joints.as_slice()[start..end]
    }

    pub fn joints_for_mut(
        &mut self,
        allocation: PerObjectJointAllocation,
    ) -> &mut [PerObjectJointTransform] {
        let start = allocation.offset as usize;
        let end = start + allocation.count as usize;
        self.dirty
            .mark_elements::<PerObjectJointTransform>(start, allocation.count as usize);
        &mut self.joints.as_slice_mut()[start..end]
    }

    pub fn joints(&self) -> &[PerObjectJointTransform] {
        self.joints.as_slice()
    }

    pub fn joints_mut(&mut self) -> &mut [PerObjectJointTransform] {
        let len = self.joints.as_slice::<PerObjectJointTransform>().len();
        self.dirty
            .mark_elements::<PerObjectJointTransform>(0, len);
        self.joints.as_slice_mut()
    }

    fn coalesce_free_ranges(&mut self) {
        if self.free_ranges.len() <= 1 {
            return;
        }

        self.free_ranges
            .sort_by_key(|range| (range.offset, range.count));

        let mut merged: Vec<PerObjectJointAllocation> = Vec::with_capacity(self.free_ranges.len());
        for range in self.free_ranges.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.offset + last.count == range.offset {
                    last.count += range.count;
                    continue;
                }
            }
            merged.push(range);
        }

        self.free_ranges = merged;
    }
}

impl ReservedItem for ReservedPerObjJoints {
    fn name(&self) -> String {
        "meshi_per_obj_joints".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.joints.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.joints.device().into()),
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
