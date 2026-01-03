#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::AnimationKeyframe};

use super::{table_binding_from_indexed, ReservedBinding, ReservedItem};

pub struct ReservedBindlessAnimationKeyframes {
    ctx: NonNull<Context>,
    keyframes: StagedBuffer,
    available_keyframes: Vec<u16>,
}

impl ReservedBindlessAnimationKeyframes {
    pub fn new(ctx: &mut Context) -> Self {
        const START_KEYFRAMES: usize = 16384;

        let available_keyframes: Vec<u16> = (0..START_KEYFRAMES as u16).collect();
        let keyframes = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Animation Keyframe Buffer",
                byte_size: std::mem::size_of::<AnimationKeyframe>() as u32
                    * START_KEYFRAMES as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            keyframes,
            available_keyframes,
        }
    }

    pub fn add_keyframe(&mut self) -> Handle<AnimationKeyframe> {
        self.available_keyframes
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn push_keyframe(&mut self, keyframe: AnimationKeyframe) -> Handle<AnimationKeyframe> {
        let handle = self.add_keyframe();
        if handle.valid() {
            *self.keyframe_mut(handle) = keyframe;
        }
        handle
    }

    pub fn remove_keyframe(&mut self, keyframe: Handle<AnimationKeyframe>) {
        if keyframe.valid()
            && (keyframe.slot as usize) < self.keyframes.as_slice::<AnimationKeyframe>().len()
        {
            self.available_keyframes.push(keyframe.slot);
        }
    }

    pub fn keyframe(&self, handle: Handle<AnimationKeyframe>) -> &AnimationKeyframe {
        &self.keyframes.as_slice()[handle.slot as usize]
    }

    pub fn keyframe_mut(&mut self, handle: Handle<AnimationKeyframe>) -> &mut AnimationKeyframe {
        &mut self.keyframes.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessAnimationKeyframes {
    fn name(&self) -> String {
        "meshi_bindless_animation_keyframes".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(self.keyframes.sync_up().end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.keyframes.device().into()),
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
