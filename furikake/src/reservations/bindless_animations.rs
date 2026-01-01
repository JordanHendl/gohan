#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{
    error::FurikakeError,
    types::{AnimationClip, AnimationKeyframe, AnimationTrack},
};

use super::{table_binding_from_indexed, ReservedBinding, ReservedItem};

pub struct ReservedBindlessAnimations {
    ctx: NonNull<Context>,
    clips: StagedBuffer,
    tracks: StagedBuffer,
    keyframes: StagedBuffer,
    available_clips: Vec<u16>,
    available_tracks: Vec<u16>,
    available_keyframes: Vec<u16>,
}

impl ReservedBindlessAnimations {
    pub fn new(ctx: &mut Context) -> Self {
        const START_CLIPS: usize = 512;
        const START_TRACKS: usize = 4096;
        const START_KEYFRAMES: usize = 16384;

        let available_clips: Vec<u16> = (0..START_CLIPS as u16).collect();
        let available_tracks: Vec<u16> = (0..START_TRACKS as u16).collect();
        let available_keyframes: Vec<u16> = (0..START_KEYFRAMES as u16).collect();
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
        let tracks = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Animation Track Buffer",
                byte_size: std::mem::size_of::<AnimationTrack>() as u32 * START_TRACKS as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );
        let keyframes = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Animation Keyframe Buffer",
                byte_size: std::mem::size_of::<AnimationKeyframe>() as u32 * START_KEYFRAMES as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            clips,
            tracks,
            keyframes,
            available_clips,
            available_tracks,
            available_keyframes,
        }
    }

    pub fn add_clip(&mut self) -> Handle<AnimationClip> {
        self.available_clips
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
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
        &mut self.clips.as_slice_mut()[handle.slot as usize]
    }

    pub fn add_track(&mut self) -> Handle<AnimationTrack> {
        self.available_tracks
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn remove_track(&mut self, track: Handle<AnimationTrack>) {
        if track.valid() && (track.slot as usize) < self.tracks.as_slice::<AnimationTrack>().len() {
            self.available_tracks.push(track.slot);
        }
    }

    pub fn track(&self, handle: Handle<AnimationTrack>) -> &AnimationTrack {
        &self.tracks.as_slice()[handle.slot as usize]
    }

    pub fn track_mut(&mut self, handle: Handle<AnimationTrack>) -> &mut AnimationTrack {
        &mut self.tracks.as_slice_mut()[handle.slot as usize]
    }

    pub fn add_keyframe(&mut self) -> Handle<AnimationKeyframe> {
        self.available_keyframes
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
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

impl ReservedItem for ReservedBindlessAnimations {
    fn name(&self) -> String {
        "meshi_bindless_animations".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(self
            .clips
            .sync_up()
            .combine(self.tracks.sync_up())
            .combine(self.keyframes.sync_up())
            .end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(self.clips.device().into()),
                    slot: 0,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(self.tracks.device().into()),
                    slot: 1,
                },
                IndexedResource {
                    resource: ShaderResource::StorageBuffer(self.keyframes.device().into()),
                    slot: 2,
                },
            ],
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
