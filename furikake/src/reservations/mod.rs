pub mod bindless_animations;
pub mod bindless_animation_keyframes;
pub mod bindless_animation_tracks;
pub mod bindless_camera;
pub mod bindless_indices;
pub mod bindless_joints;
pub mod bindless_lights;
pub mod bindless_materials;
pub mod bindless_skeletons;
pub mod bindless_skinning;
pub mod bindless_textures;
pub mod bindless_transformations;
pub mod bindless_vertices;
pub mod camera;
pub mod particles;
pub mod timing;
pub use timing::*;

use dashi::{cmd::Executable, CommandStream, Context, IndexedBindingInfo, IndexedResource};
use std::any::Any;

pub enum ReservedBinding {
    TableBinding {
        binding: u32,
        resources: Vec<IndexedResource>,
    },
}

pub trait ReservedItem {
    fn name(&self) -> String;
    fn update(&mut self) -> Result<CommandStream<Executable>, crate::error::FurikakeError>;
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
