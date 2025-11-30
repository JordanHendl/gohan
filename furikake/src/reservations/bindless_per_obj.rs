use std::ptr::NonNull;

use dashi::{
    BindingInfo, BufferInfo, Context, DynamicAllocatorInfo, DynamicBuffer, Handle,
    IndexedBindingInfo, IndexedResource, ShaderResource,
};

use crate::types::{BindlessPerObj, Camera};

use super::{ReservedBinding, ReservedItem};

pub(crate) struct ReservedBindlessPerObj {
    ctx: NonNull<Context>,
    alloc: dashi::DynamicAllocator,
}

impl ReservedBindlessPerObj {
    pub fn new(ctx: &mut Context) -> Self {
        const MAX_ALLOC: u32 = 1024;
        Self {
            ctx: NonNull::new(ctx).expect("NonNull failed check"),
            alloc: ctx
                .make_dynamic_allocator(&DynamicAllocatorInfo {
                    debug_name: "[FURIKAKE] Bindless Per Object Allocator",
                    usage: dashi::BufferUsage::UNIFORM,
                    num_allocations: MAX_ALLOC,
                    byte_size: MAX_ALLOC * std::mem::size_of::<BindlessPerObj>() as u32,
                    allocation_size: std::mem::size_of::<BindlessPerObj>() as u32,
                })
                .expect("Unable to create dynamic allocator"),
        }
    }

    pub fn bump(&mut self) -> (DynamicBuffer, NonNull<BindlessPerObj>) {
        let mut s = self.alloc.bump().expect("Failed to bump alloc");
        let ptr = NonNull::new(s.slice().as_mut_ptr()).expect("bump alloc ptr");
        (s, ptr)
    }
}

impl ReservedItem for ReservedBindlessPerObj {
    fn name(&self) -> String {
        "meshi_bindless_per_obj".to_string()
    }

    fn update(&mut self, _ctx: &mut Context) -> Result<(), crate::error::FurikakeError> {
        self.alloc.reset();
        Ok(())
    }

    fn binding(&self) -> ReservedBinding<'_> {
        return ReservedBinding::Binding(BindingInfo {
            resource: ShaderResource::Dynamic(self.alloc.state()),
            binding: 0,
        });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
