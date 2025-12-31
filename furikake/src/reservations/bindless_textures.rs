#![allow(dead_code)]

use std::{cell::RefCell, rc::Rc};

use dashi::{
    cmd::Executable, CommandStream, Context, Handle, ImageInfo, ImageView, IndexedBindingInfo,
    IndexedResource, Sampler, SamplerInfo, ShaderResource,
};

use crate::{error::FurikakeError, types::Texture};

use super::{ReservedBinding, ReservedItem, table_binding_from_indexed};

struct DefaultData {
    img: ImageView,
    sampler: Handle<Sampler>,
}

struct BindlessTextureData {
    device_image_data: Vec<IndexedResource>,
    device_sampler_data: Vec<IndexedResource>,
    host_texture_data: Vec<Texture>,
    available: Vec<u16>,
    def: DefaultData,
}

impl BindlessTextureData {
    fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 512;

        let mut d_image_data = Vec::with_capacity(START_SIZE);
        let mut d_sampler_data = Vec::with_capacity(START_SIZE);
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

            d_image_data.push(IndexedResource {
                resource: ShaderResource::Image(default_view),
                slot: i as u32,
            });
            d_sampler_data.push(IndexedResource {
                resource: ShaderResource::Sampler(default_sampler),
                slot: i as u32,
            });
        }

        Self {
            device_image_data: d_image_data,
            device_sampler_data: d_sampler_data,
            host_texture_data: h_data,
            available,
            def: DefaultData {
                img: default_view,
                sampler: default_sampler,
            },
        }
    }

    fn extend(&mut self) {
        if self.available.is_empty() {
            const EXTENSION_SIZE: usize = 128;
            let start = self.host_texture_data.len();
            let end = start + EXTENSION_SIZE;

            let default_view = self.def.img.clone();
            let default_sampler = self.def.sampler;
            for i in start..end {
                self.host_texture_data.push(Texture {
                    img: default_view,
                    sampler: Some(default_sampler),
                });

                self.device_image_data.push(IndexedResource {
                    resource: ShaderResource::Image(default_view),
                    slot: i as u32,
                });
                self.device_sampler_data.push(IndexedResource {
                    resource: ShaderResource::Sampler(default_sampler),
                    slot: i as u32,
                });

                self.available.push(i as u16);
            }
        }
    }
}

pub struct ReservedBindlessTextures {
    data: Rc<RefCell<BindlessTextureData>>,
}

pub struct ReservedBindlessSamplers {
    data: Rc<RefCell<BindlessTextureData>>,
}

impl ReservedBindlessTextures {
    pub fn new(ctx: &mut Context) -> Self {
        Self {
            data: Rc::new(RefCell::new(BindlessTextureData::new(ctx))),
        }
    }

    pub fn samplers(&self) -> ReservedBindlessSamplers {
        ReservedBindlessSamplers {
            data: Rc::clone(&self.data),
        }
    }
    
    pub fn remove_texture(&mut self, texture: u16) {
        let mut data = self.data.borrow_mut();
        let def_img = data.def.img;
        let def_sampler = data.def.sampler;
        if let Some(slot) = data.host_texture_data.get_mut(texture as usize) {
            slot.img = def_img;
            slot.sampler = Some(def_sampler);

            if let Some(resource) = data.device_image_data.get_mut(texture as usize) {
                resource.resource = ShaderResource::Image(def_img);
            }
            if let Some(resource) = data.device_sampler_data.get_mut(texture as usize) {
                resource.resource = ShaderResource::Sampler(def_sampler);
            }

            data.available.push(texture);
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
        let mut data = self.data.borrow_mut();
        if data.available.is_empty() {
            data.extend();
        }

        let id = data
            .available
            .pop()
            .expect("bindless texture slot after extension");
        let sampler = sampler.unwrap_or(data.def.sampler);

        if let Some(host) = data.host_texture_data.get_mut(id as usize) {
            host.img = img;
            host.sampler = Some(sampler);
        }

        if let Some(resource) = data.device_image_data.get_mut(id as usize) {
            resource.resource = ShaderResource::Image(img);
        }
        if let Some(resource) = data.device_sampler_data.get_mut(id as usize) {
            resource.resource = ShaderResource::Sampler(sampler);
        }

        id
    }

    pub fn update_sampler(&mut self, texture: u16, sampler: Handle<Sampler>) {
        let mut data = self.data.borrow_mut();
        if let Some(host) = data.host_texture_data.get_mut(texture as usize) {
            host.sampler = Some(sampler);

            if let Some(resource) = data.device_sampler_data.get_mut(texture as usize) {
                resource.resource = ShaderResource::Sampler(sampler);
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
        let data = self.data.borrow();
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &data.device_image_data,
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

impl ReservedItem for ReservedBindlessSamplers {
    fn name(&self) -> String {
        "meshi_bindless_samplers".to_string()
    }

    fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        Ok(CommandStream::new().begin().end())
    }

    fn binding(&self) -> ReservedBinding {
        let data = self.data.borrow();
        table_binding_from_indexed(IndexedBindingInfo {
            resources: &data.device_sampler_data,
            binding: 1,
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

        let data = textures.data.borrow();
        let host_entry = data.host_texture_data[id as usize];
        assert_eq!(host_entry.img.img, view.img);
        assert_eq!(host_entry.sampler, Some(data.def.sampler));

        match data.device_image_data[id as usize].resource {
            ShaderResource::Image(img) => {
                assert_eq!(img.img, view.img);
            }
            _ => panic!("expected image binding"),
        }
        match data.device_sampler_data[id as usize].resource {
            ShaderResource::Sampler(sampler) => {
                assert_eq!(sampler, data.def.sampler);
            }
            _ => panic!("expected sampler binding"),
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

        let data = textures.data.borrow();
        let host_entry = data.host_texture_data[id as usize];
        assert_eq!(host_entry.sampler, Some(custom_sampler));

        match data.device_sampler_data[id as usize].resource {
            ShaderResource::Sampler(sampler) => {
                assert_eq!(sampler, custom_sampler);
            }
            _ => panic!("expected sampler binding"),
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
            textures.data.borrow().host_texture_data[id as usize].sampler,
            Some(replacement_sampler)
        );
        match textures.data.borrow().device_sampler_data[id as usize].resource {
            ShaderResource::Sampler(sampler) => {
                assert_eq!(sampler, replacement_sampler);
            }
            _ => panic!("expected sampler binding"),
        }
    }

    #[test]
    fn extend_populates_available_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut textures = ReservedBindlessTextures::new(&mut ctx);
        let view = make_dummy_texture(&mut ctx, "bindless_texture_extend");

        let initial_capacity = textures.data.borrow().host_texture_data.len();

        for _ in 0..=initial_capacity {
            let _ = textures.add_texture(view);
        }

        let data = textures.data.borrow();
        assert!(data.host_texture_data.len() > initial_capacity);
        // One slot is consumed from the extension block.
        assert_eq!(data.available.len(), 127);
    }
}
