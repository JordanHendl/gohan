#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::JointTransform};

use super::{table_binding_from_indexed, ReservedBinding, ReservedItem};

pub struct ReservedBindlessJoints {
    ctx: NonNull<Context>,
    joints: StagedBuffer,
    available_joints: Vec<u16>,
}

impl ReservedBindlessJoints {
    pub fn new(ctx: &mut Context) -> Self {
        const START_JOINTS: usize = 8192;

        let available_joints: Vec<u16> = (0..START_JOINTS as u16).collect();
        let joints = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Joint Transform Buffer",
                byte_size: std::mem::size_of::<JointTransform>() as u32 * START_JOINTS as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            joints,
            available_joints,
        }
    }

    pub fn add_joint(&mut self) -> Handle<JointTransform> {
        self.available_joints
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn push_joint(&mut self, joint: JointTransform) -> Handle<JointTransform> {
        let handle = self.add_joint();
        if handle.valid() {
            *self.joint_mut(handle) = joint;
        }
        handle
    }

    pub fn remove_joint(&mut self, joint: Handle<JointTransform>) {
        if joint.valid() && (joint.slot as usize) < self.joints.as_slice::<JointTransform>().len() {
            self.available_joints.push(joint.slot);
        }
    }

    pub fn joint(&self, handle: Handle<JointTransform>) -> &JointTransform {
        &self.joints.as_slice()[handle.slot as usize]
    }

    pub fn joint_mut(&mut self, handle: Handle<JointTransform>) -> &mut JointTransform {
        &mut self.joints.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessJoints {
    fn name(&self) -> String {
        "meshi_bindless_joints".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(self.joints.sync_up().end())
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
