pub mod error;
pub mod recipe;
pub mod reservations;
pub mod resolver;
pub mod types;

use dashi::{BindGroupVariableType, Context};
use error::FurikakeError;
use reservations::{
    ReservedItem, ReservedTiming, bindless_camera::ReservedBindlessCamera,
    bindless_materials::ReservedBindlessMaterials, bindless_textures::ReservedBindlessTextures,
    bindless_transformations::ReservedBindlessTransformations,
};
use std::{collections::HashMap, ptr::NonNull};

pub use resolver::*;

pub struct ReservedMetadata {
    pub name: &'static str,
    pub kind: BindGroupVariableType,
}

pub trait GPUState {
    fn reserved_names() -> &'static [&'static str];
    fn reserved_metadata() -> &'static [ReservedMetadata];
    fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError>;
}

pub struct DefaultState {
    ctx: NonNull<Context>,
    reserved: HashMap<String, Box<dyn ReservedItem>>,
}

pub struct BindlessState {
    ctx: NonNull<Context>,
    reserved: HashMap<String, Box<dyn ReservedItem>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reservations::ReservedTiming;
    use dashi::{BufferView, ContextInfo, MemoryVisibility};
    use std::time::{Duration, Instant};

    #[repr(C)]
    struct TimingData {
        current_time_ms: f32,
        frame_time_ms: f32,
    }

    #[test]
    fn mutates_reserved_bindings_at_runtime() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut state = BindlessState::new(&mut ctx);

        state.update().expect("initial update");

        state
            .reserved_mut::<ReservedTiming, _>("meshi_timing", |timing| {
                timing.set_last_time(Instant::now() - Duration::from_millis(1200));
            })
            .expect("mutate timing");

        state.update().expect("update mutated timing");

        let timing = state
            .reserved::<ReservedTiming>("meshi_timing")
            .expect("timing reference");

        let mapped = ctx
            .map_buffer::<TimingData>(BufferView::new(timing.buffer()))
            .expect("map timing buffer");

        // Allow some wiggle room for the time spent running the test.
        assert!(mapped[0].frame_time_ms >= 1000.0);
        ctx.unmap_buffer(timing.buffer())
            .expect("unmap timing buffer after mutation");
    }

    #[test]
    fn errors_on_type_mismatch() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let mut state = BindlessState::new(&mut ctx);

        let result = state.reserved_mut::<MemoryVisibility, _>("meshi_timing", |_| {});

        match result {
            Err(FurikakeError::ReservedItemTypeMismatch { name }) => {
                assert_eq!(name, "meshi_timing")
            }
            other => panic!("expected type mismatch error, got {other:?}"),
        }
    }
}

///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///

const DEFAULT_STATE_NAMES: [&str; 1] = ["meshi_timing"];
const DEFAULT_METADATA: [ReservedMetadata; 1] = [ReservedMetadata {
    name: "meshi_timing",
    kind: BindGroupVariableType::Uniform,
}];

impl GPUState for DefaultState {
    fn reserved_names() -> &'static [&'static str] {
        DEFAULT_STATE_NAMES.as_slice()
    }

    fn reserved_metadata() -> &'static [ReservedMetadata] {
        DEFAULT_METADATA.as_slice()
    }

    fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError> {
        <DefaultState>::binding(self, key)
    }
}

impl DefaultState {
    pub fn new(ctx: &mut Context) -> Self {
        let mut reserved: HashMap<String, Box<dyn ReservedItem>> = HashMap::new();

        let names = DEFAULT_STATE_NAMES;
        reserved.insert(names[0].to_string(), Box::new(ReservedTiming::new(ctx)));

        Self {
            reserved,
            ctx: NonNull::from_ref(ctx),
        }
    }

    pub fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError> {
        if let Some(b) = self.reserved.get(key) {
            return Ok(b.as_ref());
        }

        Err(FurikakeError::MissingReservedBinding {
            name: key.to_string(),
        })
    }

    pub fn update(&mut self) -> Result<(), FurikakeError> {
        let ctx: &mut Context = unsafe { self.ctx.as_mut() };
        for iter in &mut self.reserved {
            iter.1.update(ctx)?;
        }
        Ok(())
    }

    pub fn reserved_mut<T: 'static, F: FnOnce(&mut T)>(
        &mut self,
        key: &str,
        mutate: F,
    ) -> Result<(), FurikakeError> {
        let item = self
            .reserved
            .get_mut(key)
            .ok_or(FurikakeError::MissingReservedBinding {
                name: key.to_string(),
            })?;

        let typed = item.as_any_mut().downcast_mut::<T>().ok_or(
            FurikakeError::ReservedItemTypeMismatch {
                name: key.to_string(),
            },
        )?;

        mutate(typed);
        Ok(())
    }

    pub fn reserved<T: 'static>(&self, key: &str) -> Result<&T, FurikakeError> {
        let item = self
            .reserved
            .get(key)
            .ok_or(FurikakeError::MissingReservedBinding {
                name: key.to_string(),
            })?;

        item.as_any()
            .downcast_ref::<T>()
            .ok_or(FurikakeError::ReservedItemTypeMismatch {
                name: key.to_string(),
            })
    }
}

///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////
///

const BINDLESS_STATE_NAMES: [&str; 5] = [
    "meshi_timing",
    "meshi_bindless_camera",
    "meshi_bindless_textures",
    "meshi_bindless_transformations",
    "meshi_bindless_materials",
];
const BINDLESS_METADATA: [ReservedMetadata; 5] = [
    ReservedMetadata {
        name: "meshi_timing",
        kind: BindGroupVariableType::Uniform,
    },
    ReservedMetadata {
        name: "meshi_bindless_camera",
        kind: BindGroupVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_textures",
        kind: BindGroupVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_transformations",
        kind: BindGroupVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_materials",
        kind: BindGroupVariableType::Storage,
    },
];

impl GPUState for BindlessState {
    fn reserved_names() -> &'static [&'static str] {
        BINDLESS_STATE_NAMES.as_slice()
    }

    fn reserved_metadata() -> &'static [ReservedMetadata] {
        BINDLESS_METADATA.as_slice()
    }

    fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError> {
        <BindlessState>::binding(self, key)
    }
}

impl BindlessState {
    pub fn new(ctx: &mut Context) -> Self {
        let mut reserved: HashMap<String, Box<dyn ReservedItem>> = HashMap::new();

        let names = BINDLESS_STATE_NAMES;
        reserved.insert(names[0].to_string(), Box::new(ReservedTiming::new(ctx)));
        reserved.insert(
            names[1].to_string(),
            Box::new(ReservedBindlessCamera::new(ctx)),
        );
        reserved.insert(
            names[2].to_string(),
            Box::new(ReservedBindlessTextures::new(ctx)),
        );
        reserved.insert(
            names[3].to_string(),
            Box::new(ReservedBindlessTransformations::new(ctx)),
        );
        reserved.insert(
            names[4].to_string(),
            Box::new(ReservedBindlessMaterials::new(ctx)),
        );

        Self {
            reserved,
            ctx: NonNull::from_ref(ctx),
        }
    }

    pub fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError> {
        if let Some(b) = self.reserved.get(key) {
            return Ok(b.as_ref());
        }

        Err(FurikakeError::MissingReservedBinding {
            name: key.to_string(),
        })
    }

    pub fn update(&mut self) -> Result<(), FurikakeError> {
        let ctx: &mut Context = unsafe { self.ctx.as_mut() };
        for iter in &mut self.reserved {
            iter.1.update(ctx)?;
        }
        Ok(())
    }

    pub fn reserved_mut<T: 'static, F: FnOnce(&mut T)>(
        &mut self,
        key: &str,
        mutate: F,
    ) -> Result<(), FurikakeError> {
        let item = self
            .reserved
            .get_mut(key)
            .ok_or(FurikakeError::MissingReservedBinding {
                name: key.to_string(),
            })?;

        let typed = item.as_any_mut().downcast_mut::<T>().ok_or(
            FurikakeError::ReservedItemTypeMismatch {
                name: key.to_string(),
            },
        )?;

        mutate(typed);
        Ok(())
    }

    pub fn reserved<T: 'static>(&self, key: &str) -> Result<&T, FurikakeError> {
        let item = self
            .reserved
            .get(key)
            .ok_or(FurikakeError::MissingReservedBinding {
                name: key.to_string(),
            })?;

        item.as_any()
            .downcast_ref::<T>()
            .ok_or(FurikakeError::ReservedItemTypeMismatch {
                name: key.to_string(),
            })
    }
}
