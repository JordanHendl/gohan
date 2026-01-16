#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::AnimationClip};

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

pub struct ReservedBindlessAnimations {
    ctx: NonNull<Context>,
    clips: StagedBuffer,
    available_clips: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessAnimations {
    pub fn new(ctx: &mut Context) -> Self {
        const START_CLIPS: usize = 512;

        let available_clips: Vec<u16> = (0..START_CLIPS as u16).collect();
        let clips = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Animation Clip Buffer",
                byte_size: std::mem::size_of::<AnimationClip>() as u32 * START_CLIPS as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            clips,
            available_clips,
            dirty: DirtyRange::default(),
        }
    }

    pub fn add_clip(&mut self) -> Handle<AnimationClip> {
        self.available_clips
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn push_clip(&mut self, clip: AnimationClip) -> Handle<AnimationClip> {
        let handle = self.add_clip();
        if handle.valid() {
            *self.clip_mut(handle) = clip;
        }
        handle
    }

    pub fn remove_clip(&mut self, clip: Handle<AnimationClip>) {
        if clip.valid() && (clip.slot as usize) < self.clips.as_slice::<AnimationClip>().len() {
            self.available_clips.push(clip.slot);
        }
    }

    pub fn clip(&self, handle: Handle<AnimationClip>) -> &AnimationClip {
        &self.clips.as_slice()[handle.slot as usize]
    }

    pub fn clip_mut(&mut self, handle: Handle<AnimationClip>) -> &mut AnimationClip {
        self.dirty
            .mark_elements::<AnimationClip>(handle.slot as usize, 1);
        &mut self.clips.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessAnimations {
    fn name(&self) -> String {
        "meshi_bindless_animations".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.clips.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.clips.device().into()),
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
