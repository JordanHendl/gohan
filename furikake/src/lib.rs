pub mod error;
pub mod recipe;
pub mod reservations;
pub mod resolver;
pub mod types;

use bento::builder::{BindTableUpdateTarget, CSO, CSOBuilder, PSO, PSOBuilder};
use dashi::Handle;
use dashi::{
    BindTableUpdateInfo, BindTableVariableType, CommandStream, Context, ImageView,
    IndexedBindingInfo, IndexedResource, cmd::Executable,
};

use error::FurikakeError;
use reservations::{
    ReservedItem, ReservedTiming, bindless_animation_keyframes::ReservedBindlessAnimationKeyframes,
    bindless_animation_tracks::ReservedBindlessAnimationTracks,
    bindless_animations::ReservedBindlessAnimations, bindless_camera::ReservedBindlessCamera,
    bindless_indices::ReservedBindlessIndices, bindless_joints::ReservedBindlessJoints,
    bindless_lights::ReservedBindlessLights, bindless_materials::ReservedBindlessMaterials,
    bindless_skeletons::ReservedBindlessSkeletons, bindless_skinning::ReservedBindlessSkinning,
    bindless_textures::ReservedBindlessTextures, particles::ReservedParticles,
    bindless_transformations::ReservedBindlessTransformations,
    bindless_vertices::ReservedBindlessVertices,
    per_obj_joints::ReservedPerObjJoints,
};
use std::{collections::HashMap, ptr::NonNull};
use tare::transient::BindlessTextureRegistry;
use types::{
    AnimationClip, AnimationKeyframe, AnimationState, AnimationTrack, JointTransform,
    SkeletonHeader,
};

pub use resolver::*;

pub struct ReservedMetadata {
    pub name: &'static str,
    pub kind: BindTableVariableType,
}

pub trait GPUState {
    fn reserved_names() -> &'static [&'static str];
    fn reserved_metadata() -> &'static [ReservedMetadata];
    fn binding(&self, key: &str) -> Result<&dyn ReservedItem, FurikakeError>;
}

pub trait PSOBuilderFurikakeExt {
    fn add_reserved_table_variable<T: GPUState>(
        self,
        state: &T,
        key: &str,
    ) -> Result<Self, FurikakeError>
    where
        Self: Sized;

    fn add_reserved_table_variables<T: GPUState>(self, state: &T) -> Result<Self, FurikakeError>
    where
        Self: Sized;
}

impl PSOBuilderFurikakeExt for PSOBuilder {
    fn add_reserved_table_variable<T: GPUState>(
        self,
        state: &T,
        key: &str,
    ) -> Result<Self, FurikakeError> {
        let reserved = state.binding(key)?.binding();
        let reservations::ReservedBinding::TableBinding { resources, .. } = reserved;
        Ok(self.add_table_variable_with_resources(key, resources))
    }

    fn add_reserved_table_variables<T: GPUState>(
        mut self,
        state: &T,
    ) -> Result<Self, FurikakeError> {
        for key in T::reserved_names() {
            self = self.add_reserved_table_variable(state, key)?;
        }
        Ok(self)
    }
}

impl PSOBuilderFurikakeExt for CSOBuilder {
    fn add_reserved_table_variable<T: GPUState>(
        self,
        state: &T,
        key: &str,
    ) -> Result<Self, FurikakeError> {
        let reserved = state.binding(key)?.binding();
        let reservations::ReservedBinding::TableBinding { resources, .. } = reserved;
        Ok(self.add_table_variable_with_resources(key, resources))
    }

    fn add_reserved_table_variables<T: GPUState>(
        mut self,
        state: &T,
    ) -> Result<Self, FurikakeError> {
        for key in T::reserved_names() {
            self = self.add_reserved_table_variable(state, key)?;
        }
        Ok(self)
    }
}

/// Registry for bindless animation data stored in reserved GPU buffers.
///
/// # Expected usage
/// - Register animation/skeleton assets once at load time and keep the returned `Handle<T>` IDs.
/// - Store these IDs in your database/asset layer and pass them into per-draw state (e.g. animation
///   state buffers or push constants).
/// - Avoid per-frame CPU uploads beyond state changes; only update the bindless buffers when the
///   animation or skeleton data itself changes.
pub trait BindlessAnimationRegistry {
    fn register_skeleton(&mut self) -> Handle<SkeletonHeader>;
    fn unregister_skeleton(&mut self, handle: Handle<SkeletonHeader>);
    fn register_joint(&mut self) -> Handle<JointTransform>;
    fn unregister_joint(&mut self, handle: Handle<JointTransform>);
    fn register_clip(&mut self) -> Handle<AnimationClip>;
    fn unregister_clip(&mut self, handle: Handle<AnimationClip>);
    fn register_track(&mut self) -> Handle<AnimationTrack>;
    fn unregister_track(&mut self, handle: Handle<AnimationTrack>);
    fn register_keyframe(&mut self) -> Handle<AnimationKeyframe>;
    fn unregister_keyframe(&mut self, handle: Handle<AnimationKeyframe>);
    fn register_animation_state(&mut self) -> Handle<AnimationState>;
    fn unregister_animation_state(&mut self, handle: Handle<AnimationState>);
}

pub struct DefaultState {
    ctx: NonNull<Context>,
    reserved: HashMap<String, Box<dyn ReservedItem>>,
}

pub struct BindlessState {
    ctx: NonNull<Context>,
    reserved: HashMap<String, Box<dyn ReservedItem>>,
    bind_table_subscriptions: HashMap<String, Vec<BindTableUpdateTarget>>,
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

        let mapped = timing.buffer().as_slice::<TimingData>();
        // Allow some wiggle room for the time spent running the test.
        assert!(mapped[0].frame_time_ms >= 1000.0);
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
    kind: BindTableVariableType::Uniform,
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
            iter.1.update()?;
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

const BINDLESS_STATE_NAMES: [&str; 17] = [
    "meshi_timing",
    "meshi_bindless_cameras",
    "meshi_bindless_textures",
    "meshi_bindless_samplers",
    "meshi_bindless_transformations",
    "meshi_bindless_materials",
    "meshi_bindless_lights",
    "meshi_bindless_skeletons",
    "meshi_bindless_joints",
    "meshi_bindless_animations",
    "meshi_bindless_animation_tracks",
    "meshi_bindless_animation_keyframes",
    "meshi_bindless_skinning",
    "meshi_bindless_vertices",
    "meshi_bindless_indices",
    "meshi_particles",
    "meshi_per_obj_joints",
];
const BINDLESS_METADATA: [ReservedMetadata; 17] = [
    ReservedMetadata {
        name: "meshi_timing",
        kind: BindTableVariableType::Uniform,
    },
    ReservedMetadata {
        name: "meshi_bindless_cameras",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_textures",
        kind: BindTableVariableType::Image,
    },
    ReservedMetadata {
        name: "meshi_bindless_samplers",
        kind: BindTableVariableType::Sampler,
    },
    ReservedMetadata {
        name: "meshi_bindless_transformations",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_materials",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_lights",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_skeletons",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_joints",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_animations",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_animation_tracks",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_animation_keyframes",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_skinning",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_vertices",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_bindless_indices",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_particles",
        kind: BindTableVariableType::Storage,
    },
    ReservedMetadata {
        name: "meshi_per_obj_joints",
        kind: BindTableVariableType::Storage,
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
        let textures = ReservedBindlessTextures::new(ctx);
        let samplers = textures.samplers();
        reserved.insert(names[2].to_string(), Box::new(textures));
        reserved.insert(names[3].to_string(), Box::new(samplers));
        reserved.insert(
            names[4].to_string(),
            Box::new(ReservedBindlessTransformations::new(ctx)),
        );
        reserved.insert(
            names[5].to_string(),
            Box::new(ReservedBindlessMaterials::new(ctx)),
        );
        reserved.insert(
            names[6].to_string(),
            Box::new(ReservedBindlessLights::new(ctx)),
        );
        reserved.insert(
            names[7].to_string(),
            Box::new(ReservedBindlessSkeletons::new(ctx)),
        );
        reserved.insert(
            names[8].to_string(),
            Box::new(ReservedBindlessJoints::new(ctx)),
        );
        reserved.insert(
            names[9].to_string(),
            Box::new(ReservedBindlessAnimations::new(ctx)),
        );
        reserved.insert(
            names[10].to_string(),
            Box::new(ReservedBindlessAnimationTracks::new(ctx)),
        );
        reserved.insert(
            names[11].to_string(),
            Box::new(ReservedBindlessAnimationKeyframes::new(ctx)),
        );
        reserved.insert(
            names[12].to_string(),
            Box::new(ReservedBindlessSkinning::new(ctx)),
        );
        reserved.insert(
            names[13].to_string(),
            Box::new(ReservedBindlessVertices::new(ctx)),
        );
        reserved.insert(
            names[14].to_string(),
            Box::new(ReservedBindlessIndices::new(ctx)),
        );
        reserved.insert(
            names[15].to_string(),
            Box::new(ReservedParticles::new(ctx)),
        );
        reserved.insert(
            names[16].to_string(),
            Box::new(ReservedPerObjJoints::new(ctx)),
        );

        Self {
            reserved,
            ctx: NonNull::from_ref(ctx),
            bind_table_subscriptions: HashMap::new(),
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

    pub fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
        let mut cmd = CommandStream::new().begin();
        for iter in &mut self.reserved {
            cmd = cmd.combine(iter.1.update()?);
        }
        Ok(cmd.end())
    }

    pub fn register_pso_tables(&mut self, pso: &PSO) {
        if pso.ctx.as_ptr() != self.ctx.as_ptr() {
            return;
        }

        for key in Self::reserved_names() {
            if let Some(target) = pso.table_binding(key) {
                self.register_table_target(key, target);
            }
        }
    }

    pub fn register_cso_tables(&mut self, cso: &CSO) {
        if cso.ctx.as_ptr() != self.ctx.as_ptr() {
            return;
        }

        for key in Self::reserved_names() {
            if let Some(target) = cso.table_binding(key) {
                self.register_table_target(key, target);
            }
        }
    }

    pub fn unregister_table(&mut self, key: &str, target: BindTableUpdateTarget) {
        let Some(targets) = self.bind_table_subscriptions.get_mut(key) else {
            return;
        };

        targets.retain(|existing| {
            existing.table != target.table || existing.binding != target.binding
        });

        if targets.is_empty() {
            self.bind_table_subscriptions.remove(key);
        }
    }

    fn register_table_target(&mut self, key: &str, target: BindTableUpdateTarget) {
        let targets = self
            .bind_table_subscriptions
            .entry(key.to_string())
            .or_default();
        if !targets
            .iter()
            .any(|existing| existing.table == target.table && existing.binding == target.binding)
        {
            targets.push(target);
        }
    }

    fn update_tables(&mut self, key: &str, resource: &IndexedResource) {
        let Some(targets) = self.bind_table_subscriptions.get(key).cloned() else {
            return;
        };

        let ctx = unsafe { self.ctx.as_mut() };
        let resources = std::slice::from_ref(resource);

        for target in targets {
            if resource.slot >= target.size {
                continue;
            }

            let bindings = [IndexedBindingInfo {
                resources,
                binding: target.binding,
            }];

            let _ = ctx.update_bind_table(&BindTableUpdateInfo {
                table: target.table,
                bindings: &bindings,
            });
        }
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

impl BindlessTextureRegistry for BindlessState {
    fn add_texture(&mut self, view: ImageView) -> u16 {
        let mut id = None;
        let mut image_resource = None;
        let mut sampler_resource = None;
        self.reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
            let next_id = textures.add_texture(view);
            image_resource = textures.image_resource(next_id);
            sampler_resource = textures.sampler_resource(next_id);
            id = Some(next_id);
        })
        .expect("register bindless texture in furikake");

        if let Some(resource) = image_resource.as_ref() {
            self.update_tables("meshi_bindless_textures", resource);
        }
        if let Some(resource) = sampler_resource.as_ref() {
            self.update_tables("meshi_bindless_samplers", resource);
        }

        id.expect("bindless texture id")
    }

    fn remove_texture(&mut self, id: u16) {
        let mut image_resource = None;
        let mut sampler_resource = None;
        self.reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
            textures.remove_texture(id);
            image_resource = textures.image_resource(id);
            sampler_resource = textures.sampler_resource(id);
        })
        .expect("remove bindless texture in furikake");

        if let Some(resource) = image_resource.as_ref() {
            self.update_tables("meshi_bindless_textures", resource);
        }
        if let Some(resource) = sampler_resource.as_ref() {
            self.update_tables("meshi_bindless_samplers", resource);
        }
    }
}

impl BindlessAnimationRegistry for BindlessState {
    fn register_skeleton(&mut self) -> Handle<SkeletonHeader> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessSkeletons, _>(
            "meshi_bindless_skeletons",
            |skeletons| {
                handle = Some(skeletons.add_skeleton());
            },
        )
        .expect("register bindless skeleton in furikake");
        handle.expect("bindless skeleton handle")
    }

    fn unregister_skeleton(&mut self, handle: Handle<SkeletonHeader>) {
        self.reserved_mut::<ReservedBindlessSkeletons, _>(
            "meshi_bindless_skeletons",
            |skeletons| {
                skeletons.remove_skeleton(handle);
            },
        )
        .expect("unregister bindless skeleton in furikake");
    }

    fn register_joint(&mut self) -> Handle<JointTransform> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessJoints, _>("meshi_bindless_joints", |joints| {
            handle = Some(joints.add_joint());
        })
        .expect("register bindless joint in furikake");
        handle.expect("bindless joint handle")
    }

    fn unregister_joint(&mut self, handle: Handle<JointTransform>) {
        self.reserved_mut::<ReservedBindlessJoints, _>("meshi_bindless_joints", |joints| {
            joints.remove_joint(handle);
        })
        .expect("unregister bindless joint in furikake");
    }

    fn register_clip(&mut self) -> Handle<AnimationClip> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessAnimations, _>("meshi_bindless_animations", |anims| {
            handle = Some(anims.add_clip());
        })
        .expect("register bindless animation clip in furikake");
        handle.expect("bindless animation clip handle")
    }

    fn unregister_clip(&mut self, handle: Handle<AnimationClip>) {
        self.reserved_mut::<ReservedBindlessAnimations, _>("meshi_bindless_animations", |anims| {
            anims.remove_clip(handle);
        })
        .expect("unregister bindless animation clip in furikake");
    }

    fn register_track(&mut self) -> Handle<AnimationTrack> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessAnimationTracks, _>(
            "meshi_bindless_animation_tracks",
            |tracks| {
                handle = Some(tracks.add_track());
            },
        )
        .expect("register bindless animation track in furikake");
        handle.expect("bindless animation track handle")
    }

    fn unregister_track(&mut self, handle: Handle<AnimationTrack>) {
        self.reserved_mut::<ReservedBindlessAnimationTracks, _>(
            "meshi_bindless_animation_tracks",
            |tracks| {
                tracks.remove_track(handle);
            },
        )
        .expect("unregister bindless animation track in furikake");
    }

    fn register_keyframe(&mut self) -> Handle<AnimationKeyframe> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessAnimationKeyframes, _>(
            "meshi_bindless_animation_keyframes",
            |keyframes| {
                handle = Some(keyframes.add_keyframe());
            },
        )
        .expect("register bindless animation keyframe in furikake");
        handle.expect("bindless animation keyframe handle")
    }

    fn unregister_keyframe(&mut self, handle: Handle<AnimationKeyframe>) {
        self.reserved_mut::<ReservedBindlessAnimationKeyframes, _>(
            "meshi_bindless_animation_keyframes",
            |keyframes| {
                keyframes.remove_keyframe(handle);
            },
        )
        .expect("unregister bindless animation keyframe in furikake");
    }

    fn register_animation_state(&mut self) -> Handle<AnimationState> {
        let mut handle = None;
        self.reserved_mut::<ReservedBindlessSkinning, _>("meshi_bindless_skinning", |skinning| {
            handle = Some(skinning.add_state());
        })
        .expect("register bindless animation state in furikake");
        handle.expect("bindless animation state handle")
    }

    fn unregister_animation_state(&mut self, handle: Handle<AnimationState>) {
        self.reserved_mut::<ReservedBindlessSkinning, _>("meshi_bindless_skinning", |skinning| {
            skinning.remove_state(handle);
        })
        .expect("unregister bindless animation state in furikake");
    }
}
