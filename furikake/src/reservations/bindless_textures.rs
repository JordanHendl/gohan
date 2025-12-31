#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{
    cmd::Executable, BindTable, BindTableUpdateInfo, CommandStream, Context, Handle, ImageInfo, ImageView, IndexedBindingInfo, IndexedResource, Sampler, SamplerInfo, ShaderResource
};

use crate::{error::FurikakeError, types::Texture};

use super::{ReservedBinding, ReservedItem, table_binding_from_indexed};

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

                self.available.push(i as u16);
            }
        }
    }
    
    pub fn remove_texture(&mut self, texture: u16) {
        if let Some(slot) = self.host_texture_data.get_mut(texture as usize) {
            slot.img = self.def.img;
            slot.sampler = Some(self.def.sampler);

            if let Some(resource) = self.device_texture_data.get_mut(texture as usize) {
                resource.resource = ShaderResource::SampledImage(self.def.img, self.def.sampler);
            }

            self.available.push(texture);
        }
    }

    pub fn add_texture(&mut self, img: ImageView) -> u16 {
        self.add_texture_with_sampler(img, None)
    }

    pub fn add_texture_with_sampler(
        &mut self,
        img: ImageView,
        sampler: Option<Handle<Sampler>>,
    ) -> u16 {
        if let Some(id) = self.available.pop() {
            let sampler = sampler.unwrap_or(self.def.sampler);

            if let Some(host) = self.host_texture_data.get_mut(id as usize) {
                host.img = img;
                host.sampler = Some(sampler);
            }

            if let Some(resource) = self.device_texture_data.get_mut(id as usize) {
                resource.resource = ShaderResource::SampledImage(img, sampler);
            }

            id
        } else {
            self.extend();
            self.add_texture_with_sampler(img, sampler)
        }
    }

    pub fn update_sampler(&mut self, texture: u16, sampler: Handle<Sampler>) {
        if let Some(host) = self.host_texture_data.get_mut(texture as usize) {
            host.sampler = Some(sampler);
            let img = host.img;

            if let Some(resource) = self.device_texture_data.get_mut(texture as usize) {
                resource.resource = ShaderResource::SampledImage(img, sampler);
            }
        }
    }
}

impl ReservedItem for ReservedBindlessTextures {
    fn name(&self) -> String {
        "meshi_bindless_textures".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(CommandStream::new().begin().end())
    }

    fn binding(&self) -> ReservedBinding {
        table_binding_from_indexed(IndexedBindingInfo {
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
    use dashi::{Context, ContextInfo, Format, ImageInfo, SamplerInfo};

    fn make_dummy_texture(ctx: &mut Context, name: &str) -> ImageView {
        let image = ctx
            .make_image(&ImageInfo {
                debug_name: name,
                dim: [1, 1, 1],
                format: Format::RGBA8,
                initial_data: Some(&[0, 0, 255, 255]),
                ..Default::default()
            })
            .expect("create dummy image");

        ImageView {
            img: image,
            ..Default::default()
        }
    }

    #[test]
    fn add_texture_sets_default_sampler() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);
        let view = make_dummy_texture(&mut ctx, "bindless_texture_default_sampler");

        let id = textures.add_texture(view);

        let host_entry = textures.host_texture_data[id as usize];
        assert_eq!(host_entry.img.img, view.img);
        assert_eq!(host_entry.sampler, Some(textures.def.sampler));

        match textures.device_texture_data[id as usize].resource {
            ShaderResource::SampledImage(img, sampler) => {
                assert_eq!(img.img, view.img);
                assert_eq!(sampler, textures.def.sampler);
            }
            _ => panic!("expected sampled image binding"),
        }
    }

    #[test]
    fn add_texture_with_custom_sampler_overrides_default() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);
        let view = make_dummy_texture(&mut ctx, "bindless_texture_custom_sampler");

        let custom_sampler = ctx
            .make_sampler(&SamplerInfo {
                max_anisotropy: 4.0,
                ..Default::default()
            })
            .expect("custom sampler");

        let id = textures.add_texture_with_sampler(view, Some(custom_sampler));

        let host_entry = textures.host_texture_data[id as usize];
        assert_eq!(host_entry.sampler, Some(custom_sampler));

        match textures.device_texture_data[id as usize].resource {
            ShaderResource::SampledImage(_, sampler) => {
                assert_eq!(sampler, custom_sampler);
            }
            _ => panic!("expected sampled image binding"),
        }
    }

    #[test]
    fn update_sampler_rewrites_binding() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);
        let view = make_dummy_texture(&mut ctx, "bindless_texture_update_sampler");
        let id = textures.add_texture(view);

        let replacement_sampler = ctx
            .make_sampler(&SamplerInfo {
                max_anisotropy: 8.0,
                ..Default::default()
            })
            .expect("replacement sampler");

        textures.update_sampler(id, replacement_sampler);

        assert_eq!(
            textures.host_texture_data[id as usize].sampler,
            Some(replacement_sampler)
        );
        match textures.device_texture_data[id as usize].resource {
            ShaderResource::SampledImage(_, sampler) => {
                assert_eq!(sampler, replacement_sampler);
            }
            _ => panic!("expected sampled image binding"),
        }
    }

    #[test]
    fn extend_populates_available_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);
        let view = make_dummy_texture(&mut ctx, "bindless_texture_extend");

        let initial_capacity = textures.host_texture_data.len();

        for _ in 0..=initial_capacity {
            let _ = textures.add_texture(view);
        }

        assert!(textures.host_texture_data.len() > initial_capacity);
        // One slot is consumed from the extension block.
        assert_eq!(textures.available.len(), 127);
    }
}
