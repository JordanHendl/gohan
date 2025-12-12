pub mod bindless_camera;
pub mod bindless_lights;
pub mod bindless_materials;
pub mod bindless_textures;
pub mod bindless_transformations;
pub mod camera;
pub mod timing;
pub use timing::*;

use dashi::{BindingInfo, Context, IndexedBindingInfo};
use std::any::Any;

pub enum ReservedBinding<'a> {
    Binding(BindingInfo),
    BindlessBinding(IndexedBindingInfo<'a>),
}

pub trait ReservedItem {
    fn name(&self) -> String;
    fn update(&mut self, ctx: &mut Context) -> Result<(), crate::error::FurikakeError>;
    fn binding(&self) -> ReservedBinding<'_>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
