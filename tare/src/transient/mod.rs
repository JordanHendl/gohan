use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    ptr::NonNull,
};

use dashi::*;

pub struct Ring<T, const N: usize> {
    current: usize,
    data: [T; N],
}

impl<T: Default, const N: usize> Ring<T, N> {
    pub fn new() -> Self {
        Self {
            current: 0,
            data: std::array::from_fn(|_| T::default()),
        }
    }

    pub fn new_with(data: &[T]) -> Self
    where
        T: Clone,
    {
        assert!(data.len() >= N, "not enough data to fill ring");

        Self {
            current: 0,
            data: std::array::from_fn(|idx| data[idx].clone()),
        }
    }

    pub fn set(&mut self, data: T, idx: usize) {
        let idx = idx % N;
        self.data[idx] = data;
    }

    pub fn advance(&mut self) {
        self.current = (self.current + 1) % N;
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn data(&self) -> &T {
        &self.data[self.current]
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data[self.current]
    }

    pub fn get_mut(&mut self, idx: usize) -> &mut T {
        &mut self.data[idx % N]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    dim: [u32; 3],
    layers: u32,
    format: Format,
    mip_levels: u32,
    samples: SampleCount,
}

impl From<&ImageInfo<'_>> for ImageKey {
    fn from(value: &ImageInfo<'_>) -> Self {
        Self {
            dim: value.dim,
            layers: value.layers,
            format: value.format,
            mip_levels: value.mip_levels,
            samples: value.samples,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BufferKey {
    byte_size: u32,
    visibility: MemoryVisibility,
    usage: BufferUsage,
}

impl From<&BufferInfo<'_>> for BufferKey {
    fn from(value: &BufferInfo<'_>) -> Self {
        Self {
            byte_size: value.byte_size,
            visibility: value.visibility,
            usage: value.usage,
        }
    }
}

fn hash_value<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Copy)]
struct ReuseEntry<H> {
    handle: H,
    age: usize,
}

const MAX_FRAMES: usize = 3;
const UNUSED_RETIRE_THRESHOLD: usize = MAX_FRAMES * 3;
pub struct TransientAllocator {
    ctx: NonNull<Context>,
    images: Ring<Vec<(ImageKey, Handle<Image>)>, MAX_FRAMES>,
    buffers: Ring<Vec<(BufferKey, Handle<Buffer>)>, MAX_FRAMES>,
    renderpasses: Ring<Vec<(u64, Handle<RenderPass>)>, MAX_FRAMES>,
    available_images: HashMap<ImageKey, Vec<ReuseEntry<Handle<Image>>>>,
    available_buffers: HashMap<BufferKey, Vec<ReuseEntry<Handle<Buffer>>>>,
    available_renderpasses: HashMap<u64, Vec<ReuseEntry<Handle<RenderPass>>>>,
}

impl TransientAllocator {
    pub fn new(ctx: &mut Context) -> Self {
        Self {
            ctx: NonNull::from(ctx),
            images: Ring::new(),
            buffers: Ring::new(),
            renderpasses: Ring::new(),
            available_images: HashMap::new(),
            available_buffers: HashMap::new(),
            available_renderpasses: HashMap::new(),
        }
    }

    // Helper function to check for stale data and remove it.
    fn check_for_stale(&mut self) {
        let stale_index = (self.images.current() + 1) % MAX_FRAMES;

        for (key, img) in self.images.get_mut(stale_index).drain(..) {
            self.available_images
                .entry(key)
                .or_default()
                .push(ReuseEntry {
                    handle: img,
                    age: 0,
                });
        }

        for (key, buf) in self.buffers.get_mut(stale_index).drain(..) {
            self.available_buffers
                .entry(key)
                .or_default()
                .push(ReuseEntry {
                    handle: buf,
                    age: 0,
                });
        }

        for (key, rp) in self.renderpasses.get_mut(stale_index).drain(..) {
            self.available_renderpasses
                .entry(key)
                .or_default()
                .push(ReuseEntry { handle: rp, age: 0 });
        }
    }

    fn prune_unused(&mut self) {
        let ctx = unsafe { self.ctx.as_mut() };

        self.available_images.retain(|_, list| {
            list.retain_mut(|entry| {
                entry.age += 1;
                if entry.age >= UNUSED_RETIRE_THRESHOLD {
                    ctx.destroy_image(entry.handle);
                    false
                } else {
                    true
                }
            });
            !list.is_empty()
        });

        self.available_buffers.retain(|_, list| {
            list.retain_mut(|entry| {
                entry.age += 1;
                if entry.age >= UNUSED_RETIRE_THRESHOLD {
                    ctx.destroy_buffer(entry.handle);
                    false
                } else {
                    true
                }
            });
            !list.is_empty()
        });

        self.available_renderpasses.retain(|_, list| {
            list.retain_mut(|entry| {
                entry.age += 1;
                if entry.age >= UNUSED_RETIRE_THRESHOLD {
                    ctx.destroy_render_pass(entry.handle);
                    false
                } else {
                    true
                }
            });
            !list.is_empty()
        });
    }

    pub fn advance(&mut self) {
        // advance
        self.check_for_stale();
        self.prune_unused();
        self.images.advance();
        self.buffers.advance();
        self.renderpasses.advance();
    }

    // Make a transient image matching the parameters input from this frame.
    pub fn make_image(&mut self, info: &ImageInfo) -> ImageView {
        let key = ImageKey::from(info);
        let handle = self
            .available_images
            .get_mut(&key)
            .and_then(|list| list.pop())
            .map(|entry| entry.handle)
            .unwrap_or_else(|| {
                unsafe { self.ctx.as_mut() }
                    .make_image(info)
                    .expect("Make transient image")
            });

        self.images.data_mut().push((key, handle));

        ImageView {
            img: handle,
            ..Default::default()
        }
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer(&mut self, info: &BufferInfo) -> BufferView {
        let key = BufferKey::from(info);
        let handle = self
            .available_buffers
            .get_mut(&key)
            .and_then(|list| list.pop())
            .map(|entry| entry.handle)
            .unwrap_or_else(|| {
                unsafe { self.ctx.as_mut() }
                    .make_buffer(info)
                    .expect("Make transient buffer")
            });

        self.buffers.data_mut().push((key, handle));

        BufferView::new(handle)
    }

    // Make a transient buffer matching the parameters input
    pub fn make_buffer_mapped(&mut self, info: &BufferInfo) -> (BufferView, *mut u8, u64) {
        let key = BufferKey::from(info);
        let handle = self
            .available_buffers
            .get_mut(&key)
            .and_then(|list| list.pop())
            .map(|entry| entry.handle)
            .unwrap_or_else(|| {
                unsafe { self.ctx.as_mut() }
                    .make_buffer(info)
                    .expect("Make transient buffer")
            });

        self.buffers.data_mut().push((key, handle));

        let ptr = unsafe {
            self.ctx
                .as_mut()
                .map_buffer_mut::<u8>(BufferView::new(handle))
                .unwrap()
                .as_mut_ptr()
        };
        (BufferView::new(handle), ptr, info.byte_size as u64)
    }

    pub fn make_render_pass(&mut self, info: &RenderPassInfo) -> Handle<RenderPass> {
        let hash = hash_value(info);
        let handle = self
            .available_renderpasses
            .get_mut(&hash)
            .and_then(|list| list.pop())
            .map(|entry| entry.handle)
            .unwrap_or_else(|| {
                unsafe { self.ctx.as_mut() }
                    .make_render_pass(info)
                    .expect("Make transient render pass")
            });

        self.renderpasses.data_mut().push((hash, handle));

        handle
    }
}

impl Drop for TransientAllocator {
    fn drop(&mut self) {
        // Ensure all pending resources are returned to the available pool.
        self.check_for_stale();

        let ctx = unsafe { self.ctx.as_mut() };

        for (_, img) in self.images.data.iter_mut().flatten() {
            let handle = *img;
            ctx.destroy_image(handle);
        }

        for (_, buf) in self.buffers.data.iter_mut().flatten() {
            let handle = *buf;
            ctx.destroy_buffer(handle);
        }

        for (_, rp) in self.renderpasses.data.iter_mut().flatten() {
            let handle = *rp;
            ctx.destroy_render_pass(handle);
        }

        for imgs in self.available_images.drain() {
            for entry in imgs.1 {
                ctx.destroy_image(entry.handle);
            }
        }

        for bufs in self.available_buffers.drain() {
            for entry in bufs.1 {
                ctx.destroy_buffer(entry.handle);
            }
        }

        for rps in self.available_renderpasses.drain() {
            for entry in rps.1 {
                ctx.destroy_render_pass(entry.handle);
            }
        }
    }
}
