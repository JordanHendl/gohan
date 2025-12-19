use std::time::{Duration, Instant};

use dashi::{BufferView, Context, ContextInfo, Format, ImageInfo, ImageView};
use furikake::BindlessState;
use furikake::reservations::ReservedTiming;
use furikake::reservations::bindless_camera::ReservedBindlessCamera;
use furikake::reservations::bindless_materials::ReservedBindlessMaterials;
use furikake::reservations::bindless_textures::ReservedBindlessTextures;
use furikake::reservations::bindless_transformations::ReservedBindlessTransformations;
use glam::{Mat4, Quat, Vec3};

fn make_dummy_texture(ctx: &mut Context, name: &str) -> ImageView {
    let image = ctx
        .make_image(&ImageInfo {
            debug_name: name,
            dim: [1, 1, 1],
            format: Format::RGBA8,
            initial_data: Some(&[0, 255, 0, 255]),
            ..Default::default()
        })
        .expect("create dummy image");

    ImageView {
        img: image,
        ..Default::default()
    }
}

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
            .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_cameras", |cameras| {
                let h = cameras.add_camera();
                let cam = cameras.camera_mut(h);
                cam.set_position(Vec3::new(0.0, 1.0, 2.0));
                cam.set_rotation(Quat::from_rotation_x(0.5));
                handle = Some(h);
            })
            .expect("mutate bindless camera");
        handle.expect("camera handle")
    };

    let transform_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessTransformations, _>(
                "meshi_bindless_transformations",
                |transforms| {
                    let h = transforms.add_transform();
                    transforms.transform_mut(h).transform =
                        Mat4::from_translation(Vec3::new(3.0, 4.0, 5.0));
                    handle = Some(h);
                },
            )
            .expect("mutate bindless transformation");
        handle.expect("transform handle")
    };

    let (base_tex, normal_tex, roughness_tex, occlusion_tex, emissive_tex) = {
        let base_view = make_dummy_texture(&mut ctx, "bindless_state_base");
        let normal_view = make_dummy_texture(&mut ctx, "bindless_state_normal");
        let roughness_view = make_dummy_texture(&mut ctx, "bindless_state_roughness");
        let occlusion_view = make_dummy_texture(&mut ctx, "bindless_state_occlusion");
        let emissive_view = make_dummy_texture(&mut ctx, "bindless_state_emissive");

        let mut ids = [0u16; 5];
        state
            .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
                ids[0] = textures.add_texture(base_view);
                ids[1] = textures.add_texture(normal_view);
                ids[2] = textures.add_texture(roughness_view);
                ids[3] = textures.add_texture(occlusion_view);
                ids[4] = textures.add_texture(emissive_view);
            })
            .expect("allocate bindless textures");

        (ids[0], ids[1], ids[2], ids[3], ids[4])
    };

    let material_handle = {
        let mut handle = None;
        state
            .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
                let h = materials.add_material();
                let material = materials.material_mut(h);
                material.base_color_texture_id = base_tex;
                material.normal_texture_id = normal_tex;
                material.metallic_roughness_texture_id = roughness_tex;
                material.occlusion_texture_id = occlusion_tex;
                material.emissive_texture_id = emissive_tex;
                handle = Some(h);
            })
            .expect("mutate bindless materials");
        handle.expect("material handle")
    };

    state.update().expect("flush initial mutation frame");

    {
        let cameras = state
            .reserved::<ReservedBindlessCamera>("meshi_bindless_cameras")
            .expect("camera reservation");
        let transforms = state
            .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
            .expect("transform reservation");
        let materials = state
            .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
            .expect("materials reservation");

        assert_eq!(
            cameras.camera(camera_handle).position(),
            Vec3::new(0.0, 1.0, 2.0)
        );
        assert_eq!(
            transforms
                .transform(transform_handle)
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
            (base_tex, normal_tex, roughness_tex)
        );
    }

    // Second frame: adjust every reserved binding again.
    state
        .reserved_mut::<ReservedTiming, _>("meshi_timing", |timing| {
            timing.set_last_time(Instant::now() - Duration::from_millis(220));
        })
        .expect("adjust timing delta");

    state
        .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_cameras", |cameras| {
            let cam = cameras.camera_mut(camera_handle);
            cam.set_position(Vec3::new(-1.0, 0.5, 4.0));
            cam.set_rotation(Quat::from_rotation_y(1.0));
        })
        .expect("animate camera");

    state
        .reserved_mut::<ReservedBindlessTransformations, _>(
            "meshi_bindless_transformations",
            |transforms| {
                transforms.transform_mut(transform_handle).transform =
                    Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
            },
        )
        .expect("retarget transformation");

    let (updated_base_tex, updated_normal_tex) = {
        let updated_base_view = make_dummy_texture(&mut ctx, "bindless_state_base_updated");
        let updated_normal_view = make_dummy_texture(&mut ctx, "bindless_state_normal_updated");
        let mut ids = (0u16, 0u16);
        state
            .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
                ids.0 = textures.add_texture(updated_base_view);
                ids.1 = textures.add_texture(updated_normal_view);
            })
            .expect("allocate updated textures");
        ids
    };

    state
        .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
            let material = materials.material_mut(material_handle);
            material.base_color_texture_id = updated_base_tex;
            material.normal_texture_id = updated_normal_tex;
            material.metallic_roughness_texture_id = roughness_tex;
            material.occlusion_texture_id = occlusion_tex;
            material.emissive_texture_id = emissive_tex;
        })
        .expect("swap material slots");

    state.update().expect("flush second mutation frame");

    let timing = state
        .reserved::<ReservedTiming>("meshi_timing")
        .expect("access reserved timing");
    let timing_map = timing.buffer().as_slice::<TimingData>();
    assert!(timing_map[0].frame_time_ms >= 150.0);
    assert!(timing_map[0].frame_time_ms < 500.0);

    let cameras = state
        .reserved::<ReservedBindlessCamera>("meshi_bindless_cameras")
        .expect("camera reservation");
    let transforms = state
        .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
        .expect("transform reservation");
    let materials = state
        .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
        .expect("material reservation");

    assert_eq!(
        cameras.camera(camera_handle).position(),
        Vec3::new(-1.0, 0.5, 4.0)
    );
    assert_eq!(
        transforms
            .transform(transform_handle)
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
        (
            updated_base_tex,
            updated_normal_tex,
            roughness_tex,
            occlusion_tex,
            emissive_tex,
        )
    );
}
