#![allow(dead_code)]

use std::ptr::NonNull;

use dashi::{BufferInfo, BufferView, Context, Handle, IndexedBindingInfo, IndexedResource, ShaderResource};

use crate::types::Material;

use super::{ReservedBinding, ReservedItem};

pub struct ReservedBindlessMaterials {
    ctx: NonNull<Context>,
    device_material_data: Vec<IndexedResource>,
    host_material_data: Vec<NonNull<Material>>,
    available: Vec<u16>,
}

impl ReservedBindlessMaterials {
    pub fn new(ctx: &mut Context) -> Self {
        const START_SIZE: usize = 512;

        let mut d_data = Vec::with_capacity(START_SIZE);
        let mut h_data = Vec::with_capacity(START_SIZE);
        let available: Vec<u16> = (0..START_SIZE as u16).collect();

        for i in 0..START_SIZE {
            let default = [Material::default()];
            let buf = BufferView::new(ctx
                .make_buffer(&BufferInfo {
                    debug_name: &format!("[FURIKAKE] Bindless Material {}", i),
                    byte_size: std::mem::size_of::<Material>() as u32,
                    visibility: dashi::MemoryVisibility::CpuAndGpu,
                    usage: dashi::BufferUsage::STORAGE,
                    initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                })
                .expect("Failed making material buffer"));

            let h = ctx
                .map_buffer_mut::<Material>(buf)
                .expect("Failed to map buffer");
            let nnm = NonNull::new(h.as_mut_ptr()).expect("NonNull failed check for material map!");

            h_data.push(nnm);
            d_data.push(IndexedResource {
                resource: ShaderResource::StorageBuffer(buf),
                slot: i as u32,
            });
        }

        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            device_material_data: d_data,
            host_material_data: h_data,
            available,
        }
    }

    pub fn extend(&mut self) {
        let ctx: &mut Context = unsafe { self.ctx.as_mut() };
        if self.available.is_empty() {
            const EXTENSION_SIZE: usize = 128;
            let start = self.host_material_data.len();
            let end = start + EXTENSION_SIZE;
            for i in start..end {
                let default = [Material::default()];
                let buf = BufferView::new(ctx
                    .make_buffer(&BufferInfo {
                        debug_name: &format!("[FURIKAKE] Bindless Material {}", i),
                        byte_size: std::mem::size_of::<Material>() as u32,
                        visibility: dashi::MemoryVisibility::CpuAndGpu,
                        usage: dashi::BufferUsage::STORAGE,
                        initial_data: Some(unsafe { default.align_to::<u8>().1 }),
                    })
                    .expect("Failed making material buffer"));

                let h = ctx
                    .map_buffer_mut::<Material>(buf)
                    .expect("Failed to map buffer");
                let nnm =
                    NonNull::new(h.as_mut_ptr()).expect("NonNull failed check for material map!");

                self.host_material_data.push(nnm);
                self.device_material_data.push(IndexedResource {
                    resource: ShaderResource::StorageBuffer(buf),
                    slot: i as u32,
                });
            }
        }
    }

    pub fn remove_material(&mut self, material: Handle<Material>) {
        if material.valid() && (material.slot as usize) < self.device_material_data.len() {
            self.available.push(material.slot);
        }
    }

    pub fn add_material(&mut self) -> Handle<Material> {
        if let Some(id) = self.available.pop() {
            Handle::new(id, 0)
        } else {
            self.extend();
            self.add_material()
        }
    }

    pub fn material(&self, handle: Handle<Material>) -> &Material {
        unsafe { self.host_material_data[handle.slot as usize].as_ref() }
    }

    pub fn material_mut(&mut self, handle: Handle<Material>) -> &mut Material {
        unsafe { self.host_material_data[handle.slot as usize].as_mut() }
    }
}

impl ReservedItem for ReservedBindlessMaterials {
    fn name(&self) -> String {
        "meshi_bindless_materials".to_string()
    }

    fn update(&mut self, _ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        ReservedBinding::BindlessBinding(IndexedBindingInfo {
            resources: &self.device_material_data,
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
    use crate::reservations::bindless_textures::ReservedBindlessTextures;
    use dashi::{Context, ContextInfo, Format, ImageInfo, ImageView};

    fn make_dummy_texture(ctx: &mut Context, name: &str) -> ImageView {
        let image = ctx
            .make_image(&ImageInfo {
                debug_name: name,
                dim: [1, 1, 1],
                format: Format::RGBA8,
                initial_data: Some(&[255, 0, 0, 255]),
                ..Default::default()
            })
            .expect("create dummy image");

        ImageView {
            img: image,
            ..Default::default()
        }
    }

    #[test]
    fn reuses_material_slots() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut materials = ReservedBindlessMaterials::new(&mut ctx);

        let first = materials.add_material();
        let second = materials.add_material();
        assert_ne!(first.slot, second.slot);

        materials.remove_material(first);
        let reused = materials.add_material();

        assert_eq!(first.slot, reused.slot);
    }

    #[test]
    fn stores_texture_ids_only() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut materials = ReservedBindlessMaterials::new(&mut ctx);
        let mut textures = ReservedBindlessTextures::new(&mut ctx);

        let handle = materials.add_material();
        let albedo_view = make_dummy_texture(&mut ctx, "bindless_materials_albedo");
        let normal_view = make_dummy_texture(&mut ctx, "bindless_materials_normal");
        let roughness_view = make_dummy_texture(&mut ctx, "bindless_materials_roughness");
        let occlusion_view = make_dummy_texture(&mut ctx, "bindless_materials_occlusion");
        let emissive_view = make_dummy_texture(&mut ctx, "bindless_materials_emissive");

        let albedo_id = textures.add_texture(albedo_view);
        let normal_id = textures.add_texture(normal_view);
        let roughness_id = textures.add_texture(roughness_view);
        let occlusion_id = textures.add_texture(occlusion_view);
        let emissive_id = textures.add_texture(emissive_view);
        {
            let material = materials.material_mut(handle);
            material.base_color_texture_id = albedo_id;
            material.normal_texture_id = normal_id;
            material.metallic_roughness_texture_id = roughness_id;
            material.occlusion_texture_id = occlusion_id;
            material.emissive_texture_id = emissive_id;
        }

        materials.update(&mut ctx).expect("update materials");

        let material = materials.material(handle);
        assert_eq!(material.base_color_texture_id, albedo_id);
        assert_eq!(material.normal_texture_id, normal_id);
        assert_eq!(material.metallic_roughness_texture_id, roughness_id);
        assert_eq!(material.occlusion_texture_id, occlusion_id);
        assert_eq!(material.emissive_texture_id, emissive_id);
    }
}
