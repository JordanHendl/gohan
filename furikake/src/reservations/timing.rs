use dashi::{
    cmd::Executable, Buffer, BufferInfo, BufferView, CommandStream, Context, Handle, IndexedResource, MemoryVisibility, ShaderResource
};
use tare::utils::StagedBuffer;
use std::time::Instant;

use crate::error::FurikakeError;

use super::{DirtyRange, ReservedBinding, ReservedItem};
#[repr(C)]
struct TimeData {
    current_time_ms: f32,
    frame_time_ms: f32,
}

pub struct ReservedTiming {
    last_time: Instant,
    buffer: StagedBuffer,
    dirty: DirtyRange,
}

impl ReservedTiming {
    pub fn new(ctx: &mut Context) -> Self {
        let buffer = StagedBuffer::new(ctx, BufferInfo {
                debug_name: "[FURIKAKE] Timing Buffer",
                byte_size: std::mem::size_of::<TimeData>() as u32,
                visibility: MemoryVisibility::CpuAndGpu,
                ..Default::default()
            });

        Self {
            last_time: Instant::now(),
            buffer,
            dirty: DirtyRange::default(),
        }
    }

    pub fn buffer(&self) -> StagedBuffer {
        self.buffer.clone()
    }

    pub fn set_last_time(&mut self, instant: Instant) {
        self.last_time = instant;
    }
}

impl ReservedItem for ReservedTiming {
    fn name(&self) -> String {
        "meshi_timing".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let s = self.buffer.as_slice_mut::<TimeData>();
        let now = std::time::Instant::now();
        s[0].current_time_ms = now.elapsed().as_secs_f32() * 1000.0;
        s[0].frame_time_ms = (now - self.last_time).as_secs_f32() * 1000.0;
        self.last_time = now;

        self.dirty
            .mark_elements::<TimeData>(0, self.buffer.as_slice::<TimeData>().len());
        let mut cmd = CommandStream::new().begin();
        if let Some((start, end)) = self.dirty.take() {
            cmd = cmd.combine(self.buffer.sync_up_range(start, end - start).end());
        }
        Ok(cmd.end())
    }

    fn binding(&self) -> ReservedBinding {
        ReservedBinding::TableBinding {
            binding: 0,
            resources: vec![IndexedResource {
                resource: ShaderResource::ConstBuffer(BufferView {
                    handle: self.buffer.device().handle,
                    size: (std::mem::size_of::<TimeData>) as u64,
                    offset: 0,
                }),
                slot: 0,
            }],
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
