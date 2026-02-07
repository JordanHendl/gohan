use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BufferInfo, BufferUsage, CommandStream, Context, Handle, IndexedBindingInfo,
    IndexedResource, ShaderResource,
};
use tare::utils::StagedBuffer;

use crate::{error::FurikakeError, types::ParticleState};

use super::{table_binding_from_indexed, DirtyRange, ReservedBinding, ReservedItem};

pub struct ReservedParticles {
    ctx: NonNull<Context>,
    data: StagedBuffer,
    available: Vec<u16>,
    dirty: DirtyRange,
}

impl ReservedParticles {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 16384;

        let available: Vec<u16> = (0..START_SIZE as u16).collect();
        let start = vec![ParticleState::default(); START_SIZE];
        let data = StagedBuffer::new(
            ctx,
            BufferInfo {
                debug_name: "[FURIKAKE] Particle Buffer",
                byte_size: std::mem::size_of::<ParticleState>() as u32 * START_SIZE as u32,
                visibility: Default::default(),
                usage: BufferUsage::ALL,
                initial_data: unsafe { Some(start.as_slice().align_to::<u8>().1) },
            },
        );

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            data,
            available,
            dirty: DirtyRange::default(),
        }
    }

    pub fn remove_particle(&mut self, particle: Handle<ParticleState>) {
        if particle.valid()
            && (particle.slot as usize) < self.data.as_slice::<ParticleState>().len()
        {
            self.available.push(particle.slot);
        }
    }

    pub fn add_particle(&mut self) -> Handle<ParticleState> {
        if let Some(id) = self.available.pop() {
            return Handle::new(id, 0);
        }

        Handle::new(0, 0)
    }

    pub fn push_particle(&mut self, particle: ParticleState) -> Handle<ParticleState> {
        let handle = self.add_particle();
        if handle.valid() {
            *self.particle_mut(handle) = particle;
        }
        handle
    }

    pub fn particle(&self, handle: Handle<ParticleState>) -> &ParticleState {
        &self.data.as_slice()[handle.slot as usize]
    }

    pub fn particle_mut(&mut self, handle: Handle<ParticleState>) -> &mut ParticleState {
        self.dirty
            .mark_elements::<ParticleState>(handle.slot as usize, 1);
        &mut self.data.as_slice_mut()[handle.slot as usize]
    }
}

impl ReservedItem for ReservedParticles {
    fn name(&self) -> String {
        "meshi_particles".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.data.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &[IndexedResource {
                resource: ShaderResource::StorageBuffer(self.data.device().into()),
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
