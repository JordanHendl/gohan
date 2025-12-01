use std::time::{Duration, Instant};

use dashi::{Context, ContextInfo};
use furikake::reservations::bindless_camera::ReservedBindlessCamera;
use furikake::reservations::bindless_materials::ReservedBindlessMaterials;
use furikake::reservations::bindless_textures::ReservedBindlessTextures;
use furikake::reservations::bindless_transformations::ReservedBindlessTransformations;
use furikake::reservations::ReservedTiming;
use furikake::BindlessState;
use glam::{Mat4, Quat, Vec3};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct TimingData {
    current_time_ms: f32,
    frame_time_ms: f32,
}

#[test]
fn mutates_all_bindless_reservations_across_frames() {
    let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
    let mut state = BindlessState::new(&mut ctx);

    // Seed the timing buffer so subsequent frame deltas have a baseline.
    state.update().expect("prime timing reservation");

    let camera_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
                let h = cameras.add_camera();
                let cam = cameras.camera_mut(h);
                cam.position = Vec3::new(0.0, 1.0, 2.0);
                cam.rotation = Quat::from_rotation_x(0.5);
                handle = Some(h);
            })
            .expect("mutate bindless camera");
        handle.expect("camera handle")
    };

    let texture_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
                let h = textures.add_texture();
                let tex = textures.texture_mut(h);
                tex.id = 11;
                tex.width = 800;
                tex.height = 600;
                tex.mip_levels = 4;
                handle = Some(h);
            })
            .expect("mutate bindless texture");
        handle.expect("texture handle")
    };

    let transform_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessTransformations, _>(
                "meshi_bindless_transformations",
                |transforms| {
                    let h = transforms.add_transformation();
                    transforms.transformation_mut(h).transform =
                        Mat4::from_translation(Vec3::new(3.0, 4.0, 5.0));
                    handle = Some(h);
                },
            )
            .expect("mutate bindless transformation");
        handle.expect("transform handle")
    };

    let material_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
                let h = materials.add_material();
                let material = materials.material_mut(h);
                material.base_color_texture_id = 1;
                material.normal_texture_id = 2;
                material.metallic_roughness_texture_id = 3;
                material.occlusion_texture_id = 4;
                material.emissive_texture_id = 5;
                handle = Some(h);
            })
            .expect("mutate bindless materials");
        handle.expect("material handle")
    };

    state.update().expect("flush initial mutation frame");

    {
        let cameras = state
            .reserved::<ReservedBindlessCamera>("meshi_bindless_camera")
            .expect("camera reservation");
        let textures = state
            .reserved::<ReservedBindlessTextures>("meshi_bindless_textures")
            .expect("texture reservation");
        let transforms = state
            .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
            .expect("transform reservation");
        let materials = state
            .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
            .expect("materials reservation");

        assert_eq!(cameras.camera(camera_handle).position, Vec3::new(0.0, 1.0, 2.0));
        assert_eq!(textures.texture(texture_handle).id, 11);
        assert_eq!(
            transforms
                .transformation(transform_handle)
                .transform
                .w_axis
                .truncate(),
            Vec3::new(3.0, 4.0, 5.0)
        );
        assert_eq!(
            (
                materials.material(material_handle).base_color_texture_id,
                materials.material(material_handle).normal_texture_id,
                materials
                    .material(material_handle)
                    .metallic_roughness_texture_id,
            ),
            (1, 2, 3)
        );
    }

    // Second frame: adjust every reserved binding again.
    state
        .reserved_mut::<ReservedTiming, _>("meshi_timing", |timing| {
            timing.set_last_time(Instant::now() - Duration::from_millis(220));
        })
        .expect("adjust timing delta");

    state
        .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
            let cam = cameras.camera_mut(camera_handle);
            cam.position = Vec3::new(-1.0, 0.5, 4.0);
            cam.rotation = Quat::from_rotation_y(1.0);
        })
        .expect("animate camera");

    state
        .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
            let tex = textures.texture_mut(texture_handle);
            tex.id = 27;
            tex.width = 2048;
            tex.height = 1024;
        })
        .expect("swap texture metadata");

    state
        .reserved_mut::<ReservedBindlessTransformations, _>(
            "meshi_bindless_transformations",
            |transforms| {
                transforms.transformation_mut(transform_handle).transform =
                    Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
            },
        )
        .expect("retarget transformation");

    state
        .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
            let material = materials.material_mut(material_handle);
            material.base_color_texture_id = 7;
            material.normal_texture_id = 8;
            material.metallic_roughness_texture_id = 9;
            material.occlusion_texture_id = 10;
            material.emissive_texture_id = 11;
        })
        .expect("swap material slots");

    state.update().expect("flush second mutation frame");

    let timing = state
        .reserved::<ReservedTiming>("meshi_timing")
        .expect("access reserved timing");
    let timing_map = ctx
        .map_buffer::<TimingData>(timing.buffer())
        .expect("map timing");
    assert!(timing_map[0].frame_time_ms >= 150.0);
    assert!(timing_map[0].frame_time_ms < 500.0);
    ctx.unmap_buffer(timing.buffer()).expect("unmap timing");

    let cameras = state
        .reserved::<ReservedBindlessCamera>("meshi_bindless_camera")
        .expect("camera reservation");
    let textures = state
        .reserved::<ReservedBindlessTextures>("meshi_bindless_textures")
        .expect("texture reservation");
    let transforms = state
        .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
        .expect("transform reservation");
    let materials = state
        .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
        .expect("material reservation");

    assert_eq!(cameras.camera(camera_handle).position, Vec3::new(-1.0, 0.5, 4.0));
    assert_eq!(textures.texture(texture_handle).id, 27);
    assert_eq!(textures.texture(texture_handle).width, 2048);
    assert_eq!(textures.texture(texture_handle).height, 1024);
    assert_eq!(
        transforms
            .transformation(transform_handle)
            .transform
            .w_axis
            .truncate(),
        Vec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(
        (
            materials.material(material_handle).base_color_texture_id,
            materials.material(material_handle).normal_texture_id,
            materials
                .material(material_handle)
                .metallic_roughness_texture_id,
            materials.material(material_handle).occlusion_texture_id,
            materials.material(material_handle).emissive_texture_id,
        ),
        (7, 8, 9, 10, 11)
    );
}
