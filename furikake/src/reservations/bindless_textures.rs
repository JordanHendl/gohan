#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    BufferInfo, Context, Handle, ImageInfo, ImageView, IndexedBindingInfo, IndexedResource,
    Sampler, SamplerInfo, ShaderResource,
};

use crate::types::Texture;

use super::{ReservedBinding, ReservedItem};

struct DefaultData {
    img: ImageView,
    sampler: Handle<Sampler>,
}
pub struct ReservedBindlessTextures {
    ctx: NonNull<Context>,
    device_texture_data: Vec<IndexedResource>,
    host_texture_data: Vec<Texture>,
    available: Vec<u16>,
    def: DefaultData,
}

impl ReservedBindlessTextures {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 512;

        let mut d_data = Vec::with_capacity(START_SIZE);
        let mut h_data = Vec::with_capacity(START_SIZE);
        let available: Vec<u16> = (0..START_SIZE as u16).collect();

        let default_sampler = ctx.make_sampler(&SamplerInfo::default()).unwrap();
        let default_image = ctx
            .make_image(&ImageInfo {
                debug_name: "[FURIKAKE] Default Image",
                dim: [1, 1, 1],
                format: dashi::Format::RGBA8,
                initial_data: Some(&[128, 128, 0, 255]),
                ..Default::default()
            })
            .unwrap();

        let default_view = ImageView {
            img: default_image,
            ..Default::default()
        };

        for i in 0..START_SIZE {
            h_data.push(Texture {
                img: default_view,
                sampler: Some(default_sampler),
            });

            d_data.push(IndexedResource {
                resource: ShaderResource::SampledImage(default_view, default_sampler),
                slot: i as u32,
            });
        }

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            device_texture_data: d_data,
            host_texture_data: h_data,
            available,
            def: DefaultData {
                img: default_view,
                sampler: default_sampler,
            },
        }
    }

    pub fn extend(&mut self) {
        if self.available.is_empty() {
            const EXTENSION_SIZE: usize = 128;
            let start = self.host_texture_data.len();
            let end = start + EXTENSION_SIZE;

            let default_view = self.def.img.clone();
            let default_sampler = self.def.sampler.clone();
            for i in start..end {
                self.host_texture_data.push(Texture {
                    img: default_view,
                    sampler: Some(default_sampler),
                });

                self.device_texture_data.push(IndexedResource {
                    resource: ShaderResource::SampledImage(default_view, default_sampler),
                    slot: i as u32,
                });
            }
        }
    }

    pub fn remove_texture(&mut self, texture: u16) {
        self.available.push(texture);
    }

    pub fn add_texture(&mut self, img: ImageView) -> u16 {
        if let Some(id) = self.available.pop() {
            id
        } else {
            self.extend();
            self.add_texture(img)
        }
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
