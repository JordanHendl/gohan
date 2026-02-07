#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::AnimationState};

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

pub struct ReservedBindlessSkinning {
    ctx: NonNull<Context>,
    states: StagedBuffer,
    available_states: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedBindlessSkinning {
    pub fn new(ctx: &mut Context) -> Self {
        const START_STATES: usize = 4096;

        let available_states: Vec<u16> = (0..START_STATES as u16).collect();
        let states = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Animation State Buffer",
                byte_size: std::mem::size_of::<AnimationState>() as u32 * START_STATES as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: None,
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            states,
            available_states,
            dirty: DirtyRange::default(),
        }
    }

    pub fn add_state(&mut self) -> Handle<AnimationState> {
        self.available_states
            .pop()
            .map(|id| Handle::new(id, 0))
            .unwrap_or_else(|| Handle::new(u16::MAX, u16::MAX))
    }

    pub fn push_state(&mut self, state: AnimationState) -> Handle<AnimationState> {
        let handle = self.add_state();
        if handle.valid() {
            *self.state_mut(handle) = state;
        }
        handle
    }

    pub fn remove_state(&mut self, state: Handle<AnimationState>) {
        if state.valid() && (state.slot as usize) < self.states.as_slice::<AnimationState>().len()
        {
            self.available_states.push(state.slot);
        }
    }

    pub fn state(&self, handle: Handle<AnimationState>) -> &AnimationState {
        &self.states.as_slice()[handle.slot as usize]
    }

    pub fn state_mut(&mut self, handle: Handle<AnimationState>) -> &mut AnimationState {
        self.dirty
            .mark_elements::<AnimationState>(handle.slot as usize, 1);
        &mut self.states.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedBindlessSkinning {
    fn name(&self) -> String {
        "meshi_bindless_skinning".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.states.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.states.device().into()),
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

mod test {
    #[test]
    fn ensure_size_of_state() {
        use crate::types::*;

        assert_eq!(std::mem::size_of::<AnimationState>(), 32);
    }
}
