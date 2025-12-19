use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use dashi::{BufferView, Context, ContextInfo, Format, ImageInfo, ImageView, ShaderType};
use furikake::recipe::RecipeBook;
use furikake::reservations::ReservedTiming;
use furikake::reservations::bindless_camera::ReservedBindlessCamera;
use furikake::reservations::bindless_materials::ReservedBindlessMaterials;
use furikake::reservations::bindless_textures::ReservedBindlessTextures;
use furikake::reservations::bindless_transformations::ReservedBindlessTransformations;
use furikake::{BindlessState, Resolver};
use glam::{Mat4, Quat, Vec3};
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct TimingData {
    current_time_ms: f32,
    frame_time_ms: f32,
}

fn make_solid_texture(ctx: &mut Context, name: &str, color: [u8; 4]) -> ImageView {
    let image = ctx
        .make_image(&ImageInfo {
            debug_name: name,
            dim: [1, 1, 1],
            format: Format::RGBA8,
            initial_data: Some(&color),
            ..Default::default()
        })
        .expect("create example image");

    ImageView {
        img: image,
        ..Default::default()
    }
}

fn compile_shader() -> bento::CompilationResult {
    let source = r#"
        #version 450 core
        layout(local_size_x = 1) in;

        layout(set = 0, binding = 0) uniform timing {
            float current_time_ms;
            float frame_time_ms;
        } meshi_timing;

        struct Camera {
            mat4 world_from_camera;
            mat4 projection;
            vec2 viewport;
            float near;
            float far;
            float fov_y_radians;
            uint projection_kind;
            float _padding0;
        };
        layout(set = 1, binding = 0) buffer Cameras {
            Camera cameras[];
        } meshi_bindless_camera;

        struct Texture {
            uint id;
            uint width;
            uint height;
            uint mip_levels;
        };
        layout(set = 2, binding = 0) uniform Sampler2D meshi_bindless_textures[]; 

        layout(set = 3, binding = 0) buffer Transformations {
            mat4 transforms[];
        } meshi_bindless_transformations;

        struct Material {
            uint base_color_texture_id;
            uint normal_texture_id;
            uint metallic_roughness_texture_id;
            uint occlusion_texture_id;
            uint emissive_texture_id;
            uint _padding;
        };
        layout(set = 4, binding = 0) buffer Materials {
            Material materials[];
        } meshi_bindless_materials;

        void main() {
            float time_mix = meshi_timing.frame_time_ms * 0.001;
            vec3 camera_dir = normalize(-meshi_bindless_camera.cameras[0].world_from_camera[2].xyz);
            uint texture_id = meshi_bindless_textures.textures[0].id;
            mat4 model = meshi_bindless_transformations.transforms[0];
            uint material_tex = meshi_bindless_materials.materials[0].base_color_texture_id;

            if (time_mix + camera_dir.x + float(texture_id + material_tex) + model[0][0] > -1.0) {
                // reference everything so the compiler keeps all reserved bindings
            }
        }
    "#;

    let compiler = Compiler::new().expect("create bento compiler");
    compiler
        .compile(
            source.as_bytes(),
            &Request {
                name: Some("bindless_reserved_compute".to_string()),
                lang: ShaderLang::Glsl,
                stage: ShaderType::Compute,
                optimization: OptimizationLevel::None,
                debug_symbols: true,
                ..Default::default()
            },
        )
        .expect("compile compute shader")
}

fn main() {
    let mut ctx = Context::headless(&ContextInfo::default()).expect("create dashi context");
    let mut state = BindlessState::new(&mut ctx);

    let shader = compile_shader();
    let resolver = Resolver::new(&state, &shader).expect("reflect all reserved bindings");
    println!(
        "Validated bindless reserved bindings: {:?}",
        resolver.resolved()
    );

    let book = RecipeBook::new(&mut ctx, &state, &[shader]).expect("build recipe book");
    let mut bt_recipes = book.recipes();
    println!("Bind table recipes: {}", bt_recipes.len());
    for recipe in &bt_recipes {
        let set = recipe.bindings.first().map(|b| b.var.set).unwrap_or(0);
        println!(
            "  set {set} -> {} indexed binding(s)",
            recipe.bindings.len()
        );
    }

    // Allocate and mutate every bindless reservation.
    let mut camera_handle = None;
    state
        .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
            let handle = cameras.add_camera();
            let camera = cameras.camera_mut(handle);
            *camera =
                furikake::types::Camera::new(Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_y(1.2));
            camera_handle = Some(handle);
        })
        .expect("mutate cameras");

    let mut transform_handle = None;
    state
        .reserved_mut::<ReservedBindlessTransformations, _>(
            "meshi_bindless_transformations",
            |transforms| {
                let handle = transforms.add_transform();
                transforms.transform_mut(handle).transform =
                    Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0));
                transform_handle = Some(handle);
            },
        )
        .expect("mutate transformations");

    let base_texture = make_solid_texture(&mut ctx, "bindless_reserved_base", [255, 0, 0, 255]);
    let normal_texture = make_solid_texture(&mut ctx, "bindless_reserved_normal", [0, 255, 0, 255]);
    let roughness_texture =
        make_solid_texture(&mut ctx, "bindless_reserved_roughness", [0, 0, 255, 255]);
    let occlusion_texture =
        make_solid_texture(&mut ctx, "bindless_reserved_occlusion", [255, 255, 0, 255]);
    let emissive_texture =
        make_solid_texture(&mut ctx, "bindless_reserved_emissive", [255, 0, 255, 255]);

    let mut texture_ids = [0u16; 5];
    state
        .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
            texture_ids[0] = textures.add_texture(base_texture);
            texture_ids[1] = textures.add_texture(normal_texture);
            texture_ids[2] = textures.add_texture(roughness_texture);
            texture_ids[3] = textures.add_texture(occlusion_texture);
            texture_ids[4] = textures.add_texture(emissive_texture);
        })
        .expect("add bindless textures");

    let mut material_handle = None;
    state
        .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
            let handle = materials.add_material();
            let material = materials.material_mut(handle);
            material.base_color_texture_id = texture_ids[0];
            material.normal_texture_id = texture_ids[1];
            material.metallic_roughness_texture_id = texture_ids[2];
            material.occlusion_texture_id = texture_ids[3];
            material.emissive_texture_id = texture_ids[4];
            material_handle = Some(handle);
        })
        .expect("mutate materials");

    // Drive timing forward and flush any host-side changes.
    state.update().expect("refresh reserved state");

    let timing = state
        .reserved::<ReservedTiming>("meshi_timing")
        .expect("access reserved timing");
    let timing_map = timing.buffer().as_slice::<TimingData>();
    println!(
        "Timing snapshot -> current: {:.3}ms | frame: {:.3}ms",
        timing_map[0].current_time_ms, timing_map[0].frame_time_ms
    );

    let camera_handle = camera_handle.expect("camera handle");
    let transform_handle = transform_handle.expect("transform handle");
    let material_handle = material_handle.expect("material handle");

    {
        let cameras = state
            .reserved::<ReservedBindlessCamera>("meshi_bindless_camera")
            .expect("camera reservation");
        let transforms = state
            .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
            .expect("transform reservation");
        let materials = state
            .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
            .expect("material reservation");

        println!(
            "Camera[{}] position: {:?}",
            camera_handle.slot,
            cameras.camera(camera_handle).position()
        );
        println!(
            "Transform[{}] translation: {:?}",
            transform_handle.slot,
            transforms
                .transform(transform_handle)
                .transform
                .w_axis
                .truncate()
        );
        println!(
            "Material[{}] texture ids: base={} normal={} m/r={} occ={} emissive={}",
            material_handle.slot,
            materials.material(material_handle).base_color_texture_id,
            materials.material(material_handle).normal_texture_id,
            materials
                .material(material_handle)
                .metallic_roughness_texture_id,
            materials.material(material_handle).occlusion_texture_id,
            materials.material(material_handle).emissive_texture_id,
        );
    }

    // Simulate a later frame where we tweak every reservation again.
    state
        .reserved_mut::<ReservedTiming, _>("meshi_timing", |timing| {
            // Pretend the last frame ended 240ms ago.
            timing.set_last_time(Instant::now() - Duration::from_millis(240));
        })
        .expect("backdate timing");

    state
        .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
            let cam = cameras.camera_mut(camera_handle);
            cam.set_position(cam.position() + Vec3::new(0.5, -0.25, 1.0));
            cam.set_rotation(Quat::from_rotation_y(1.57));
        })
        .expect("tweak camera per-frame");

    state
        .reserved_mut::<ReservedBindlessTransformations, _>(
            "meshi_bindless_transformations",
            |transforms| {
                let transform = transforms.transform_mut(transform_handle);
                transform.transform =
                    Mat4::from_rotation_z(0.5) * Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
            },
        )
        .expect("refresh transform per-frame");

    let updated_base = make_solid_texture(
        &mut ctx,
        "bindless_reserved_base_updated",
        [0, 255, 255, 255],
    );
    let updated_normal = make_solid_texture(
        &mut ctx,
        "bindless_reserved_normal_updated",
        [64, 64, 64, 255],
    );

    let mut updated_ids = (texture_ids[0], texture_ids[1]);
    state
        .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
            updated_ids.0 = textures.add_texture(updated_base);
            updated_ids.1 = textures.add_texture(updated_normal);
        })
        .expect("allocate updated textures");

    state
        .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
            let material = materials.material_mut(material_handle);
            material.base_color_texture_id = updated_ids.0;
            material.normal_texture_id = updated_ids.1;
        })
        .expect("adjust material bindings");

    state.update().expect("flush second round of edits");

    let timing = state
        .reserved::<ReservedTiming>("meshi_timing")
        .expect("access reserved timing");
    let timing_map = timing.buffer().as_slice::<TimingData>();
    println!(
        "After runtime edits -> current: {:.3}ms | frame: {:.3}ms",
        timing_map[0].current_time_ms, timing_map[0].frame_time_ms
    );

    let cameras = state
        .reserved::<ReservedBindlessCamera>("meshi_bindless_camera")
        .expect("camera reservation");
    let transforms = state
        .reserved::<ReservedBindlessTransformations>("meshi_bindless_transformations")
        .expect("transform reservation");
    let materials = state
        .reserved::<ReservedBindlessMaterials>("meshi_bindless_materials")
        .expect("material reservation");

    println!(
        "Camera[{}] position (mutated): {:?}",
        camera_handle.slot,
        cameras.camera(camera_handle).position()
    );
    println!(
        "Transform[{}] translation after runtime edit: {:?}",
        transform_handle.slot,
        transforms
            .transform(transform_handle)
            .transform
            .w_axis
            .truncate()
    );
    println!(
        "Material[{}] texture ids after runtime edit: base={} normal={} m/r={} occ={} emissive={}",
        material_handle.slot,
        materials.material(material_handle).base_color_texture_id,
        materials.material(material_handle).normal_texture_id,
        materials
            .material(material_handle)
            .metallic_roughness_texture_id,
        materials.material(material_handle).occlusion_texture_id,
        materials.material(material_handle).emissive_texture_id,
    );

    // Cook the bindless resources after we've populated data to mirror real usage.
    for mut recipe in bt_recipes.drain(..) {
        let _table = recipe.cook(&mut ctx).expect("cook bind table");
    }
}
