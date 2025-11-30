use dashi::{
    BindingInfo, Buffer, BufferInfo, BufferView, Context, Handle, MemoryVisibility, ShaderResource,
};
use std::time::Instant;

use super::{ReservedBinding, ReservedItem};
#[repr(C)]
struct TimeData {
    current_time_ms: f32,
    frame_time_ms: f32,
}

pub struct ReservedTiming {
    last_time: Instant,
    buffer: Handle<Buffer>,
}

impl ReservedTiming {
    pub fn new(ctx: &mut Context) -> Self {
        let buffer = ctx
            .make_buffer(&BufferInfo {
                debug_name: "[FURIKAKE] Timing Buffer",
                byte_size: std::mem::size_of::<TimeData>() as u32,
                visibility: MemoryVisibility::CpuAndGpu,
                ..Default::default()
            })
            .expect("Unable to make timing buffer!");

        Self {
            last_time: Instant::now(),
            buffer,
        }
    }

    pub fn buffer(&self) -> Handle<Buffer> {
        self.buffer
    }

    pub fn set_last_time(&mut self, instant: Instant) {
        self.last_time = instant;
    }
}

impl ReservedItem for ReservedTiming {
    fn name(&self) -> String {
        "meshi_timing".to_string()
    }

    fn update(&mut self, ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        let s = ctx
            .map_buffer_mut::<TimeData>(self.buffer)
            .map_err(crate::error::FurikakeError::buffer_map_failed)?;
        let now = std::time::Instant::now();
        s[0].current_time_ms = now.elapsed().as_secs_f32() * 1000.0;
        s[0].frame_time_ms = (now - self.last_time).as_secs_f32() * 1000.0;
        self.last_time = now;
        ctx.unmap_buffer(self.buffer)
            .map_err(crate::error::FurikakeError::buffer_unmap_failed)?;

        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        return ReservedBinding::Binding(BindingInfo {
            resource: ShaderResource::ConstBuffer(BufferView {
                handle: self.buffer,
                size: (std::mem::size_of::<f32>() * 2) as u64,
                offset: 0,
            }),
            binding: 0,
        });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
