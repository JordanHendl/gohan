#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::AnimationTrack};

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

pub struct ReservedBindlessAnimationTracks {
    ctx: NonNull<Context>,
    tracks: StagedBuffer,
    available_tracks: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessAnimationTracks {
    pub fn new(ctx: &mut Context) -> Self {
        const START_TRACKS: usize = 4096;

        let available_tracks: Vec<u16> = (0..START_TRACKS as u16).collect();
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

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            tracks,
            available_tracks,
            dirty: DirtyRange::default(),
        }
    }

    pub fn add_track(&mut self) -> Handle<AnimationTrack> {
        self.available_tracks
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn push_track(&mut self, track: AnimationTrack) -> Handle<AnimationTrack> {
        let handle = self.add_track();
        if handle.valid() {
            *self.track_mut(handle) = track;
        }
        handle
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
        self.dirty
            .mark_elements::<AnimationTrack>(handle.slot as usize, 1);
        &mut self.tracks.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessAnimationTracks {
    fn name(&self) -> String {
        "meshi_bindless_animation_tracks".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.tracks.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.tracks.device().into()),
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
