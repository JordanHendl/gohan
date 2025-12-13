use cmd::Recording;
use dashi::*;
use driver::command::CopyBuffer;

use crate::transient::TransientAllocator;

pub struct StagedBuffer {
    device: BufferView,
    host: BufferView,
    mapped: (*mut u8, usize),
}

impl Default for StagedBuffer {
    fn default() -> Self {
        Self { device: BufferView::new(Default::default()), host: BufferView::new(Default::default()), mapped: Default::default() }
    }
}
impl StagedBuffer {
    pub fn new(ctx: &mut Context, info: BufferInfo) -> Self {
        let mut info = info.clone();

        info.visibility = MemoryVisibility::Gpu;

        let device = ctx
            .make_buffer(&info)
            .expect("Unable to make device buffer!");

        info.visibility = MemoryVisibility::CpuAndGpu;

        let host = ctx
            .make_buffer(&info)
            .expect("Unable to make host staging buffer!");

        let mapped = (
            ctx.map_buffer_mut::<u8>(BufferView::new(host))
                .expect("Unable to map host buffer")
                .as_mut_ptr(),
            info.byte_size as usize,
        );
        return Self {
            device: BufferView::new(device),
            host: BufferView::new(host),
            mapped,
        };
    }

    pub fn new_transient(ctx: &mut TransientAllocator, info: BufferInfo) -> Self {
        let mut info = info.clone();

        info.visibility = MemoryVisibility::Gpu;

        let device = ctx.make_buffer(&info);

        info.visibility = MemoryVisibility::CpuAndGpu;

        let (host, ptr, len) = ctx.make_buffer_mapped(&info);

        return Self {
            device,
            host,
            mapped: (ptr, len as usize),
        };
    }

    pub fn device(&self) -> BufferView {
        self.device
    }

    pub fn host(&self) -> BufferView {
        self.host
    }

    pub fn as_slice<T>(&mut self) -> &'static [T] {
        let (ptr, len) = self.mapped;
        let ptr = ptr as *const T;
        let len = len / std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    pub fn as_slice_mut<T>(&mut self) -> &'static mut [T] {
        let (ptr, len) = self.mapped;
        let ptr = ptr as *mut T;
        let len = len / std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    pub fn sync_up(&self) -> CommandStream<Recording> {
        let mut cmd = CommandStream::new().begin();

        cmd.copy_buffers(&CopyBuffer {
            src: self.host.handle,
            dst: self.device.handle,
            ..Default::default()
        });

        cmd
    }

    pub fn sync_down(&self) -> CommandStream<Recording> {
        let mut cmd = CommandStream::new().begin();

        cmd.copy_buffers(&CopyBuffer {
            src: self.device.handle,
            dst: self.host.handle,
            ..Default::default()
        });

        cmd
    }
}
