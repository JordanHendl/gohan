pub mod bindless_camera;
pub mod bindless_lights;
pub mod bindless_materials;
pub mod bindless_textures;
pub mod bindless_transformations;
pub mod camera;
pub mod timing;
pub use timing::*;

use dashi::{Context, IndexedBindingInfo, IndexedResource};
use std::any::Any;

pub enum ReservedBinding {
    TableBinding {
        binding: u32,
        resources: Vec<IndexedResource>,
    },
}

pub trait ReservedItem {
    fn name(&self) -> String;
    fn update(&mut self, ctx: &mut Context) -> Result<(), crate::error::FurikakeError>;
    fn binding(&self) -> ReservedBinding;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) fn table_binding_from_indexed(info: IndexedBindingInfo<'_>) -> ReservedBinding {
    ReservedBinding::TableBinding {
        binding: info.binding,
        resources: info.resources.to_vec(),
    }
}
