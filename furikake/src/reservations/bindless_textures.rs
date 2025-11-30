#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{BufferInfo, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource};

use crate::types::Texture;

use super::{ReservedBinding, ReservedItem};

pub struct ReservedBindlessTextures {
    ctx: NonNull<Context>,
    device_texture_data: Vec<IndexedResource>,
    host_texture_data: Vec<NonNull<Texture>>,
    available: Vec<u16>,
}

impl ReservedBindlessTextures {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 512;

        let mut d_data = Vec::with_capacity(START_SIZE);
        let mut h_data = Vec::with_capacity(START_SIZE);
        let available: Vec<u16> = (0..START_SIZE as u16).collect();

        for i in 0..START_SIZE {
            let default = [Texture::default()];
            let buf = ctx
                .make_buffer(&BufferInfo {
                    debug_name: &format!("[FURIKAKE] Bindless Texture {}", i),
                    byte_size: std::mem::size_of::<Texture>() as u32,
                    visibility: dashi::MemoryVisibility::CpuAndGpu,
                    usage: dashi::BufferUsage::STORAGE,
                    initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                })
                .expect("Failed making texture buffer");

            let h = ctx
                .map_buffer_mut::<Texture>(buf)
                .expect("Failed to map buffer");
            let nnt = NonNull::new(h.as_mut_ptr()).expect("NonNull failed check for texture map!");

            h_data.push(nnt);
            d_data.push(IndexedResource {
                resource: ShaderResource::StorageBuffer(buf),
                slot: i as u32,
            });
        }

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            device_texture_data: d_data,
            host_texture_data: h_data,
            available,
        }
    }

    pub fn extend(&mut self) {
        let ctx: &mut Context = unsafe { self.ctx.as_mut() };
        if self.available.is_empty() {
            const EXTENSION_SIZE: usize = 128;
            let start = self.host_texture_data.len();
            let end = start + EXTENSION_SIZE;
            for i in start..end {
                let default = [Texture::default()];
                let buf = ctx
                    .make_buffer(&BufferInfo {
                        debug_name: &format!("[FURIKAKE] Bindless Texture {}", i),
                        byte_size: std::mem::size_of::<Texture>() as u32,
                        visibility: dashi::MemoryVisibility::CpuAndGpu,
                        usage: dashi::BufferUsage::STORAGE,
                        initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                    })
                    .expect("Failed making texture buffer");

                let h = ctx
                    .map_buffer_mut::<Texture>(buf)
                    .expect("Failed to map buffer");
                let nnt =
                    NonNull::new(h.as_mut_ptr()).expect("NonNull failed check for texture map!");

                self.host_texture_data.push(nnt);
                self.device_texture_data.push(IndexedResource {
                    resource: ShaderResource::StorageBuffer(buf),
                    slot: i as u32,
                });
            }
        }
    }

    pub fn remove_texture(&mut self, texture: Handle<Texture>) {
        if texture.valid() && (texture.slot as usize) < self.device_texture_data.len() {
            self.available.push(texture.slot);
        }
    }

    pub fn add_texture(&mut self) -> Handle<Texture> {
        if let Some(id) = self.available.pop() {
            Handle::new(id, 0)
        } else {
            self.extend();
            self.add_texture()
        }
    }

    pub fn texture(&self, handle: Handle<Texture>) -> &Texture {
        unsafe { self.host_texture_data[handle.slot as usize].as_ref() }
    }

    pub fn texture_mut(&mut self, handle: Handle<Texture>) -> &mut Texture {
        unsafe { self.host_texture_data[handle.slot as usize].as_mut() }
    }
}

impl ReservedItem for ReservedBindlessTextures {
    fn name(&self) -> String {
        "meshi_bindless_textures".to_string()
    }

    fn update(&mut self, _ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        ReservedBinding::BindlessBinding(IndexedBindingInfo {
            resources: &self.device_texture_data,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dashi::{Context, ContextInfo};

    #[test]
    fn reuses_texture_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);

        let first = textures.add_texture();
        let second = textures.add_texture();
        assert_ne!(first.slot, second.slot);

        textures.remove_texture(first);
        let reused = textures.add_texture();

        assert_eq!(first.slot, reused.slot);
    }

    #[test]
    fn writes_texture_metadata() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);

        let handle = textures.add_texture();
        {
            let texture = textures.texture_mut(handle);
            texture.id = 42;
            texture.width = 1024;
            texture.height = 512;
            texture.mip_levels = 5;
        }

        textures.update(&mut ctx).expect("update textures");

        let texture = textures.texture(handle);
        assert_eq!(texture.id, 42);
        assert_eq!(texture.width, 1024);
        assert_eq!(texture.height, 512);
        assert_eq!(texture.mip_levels, 5);
    }
}
