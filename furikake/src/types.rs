use dashi::{Handle, Image, ImageView, Sampler};
use glam::{Mat4, Quat, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(Vec3::ZERO, Quat::IDENTITY)
    }
}

impl Camera {
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self { position, rotation }
    }

    /// Point the camera at a target world position.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let forward = (target - self.position).normalize();

        // glam's look_to_rh gives a view matrix (world → camera)
        let view = Mat4::look_to_rh(self.position, forward, up);

        // Convert view → camera transform → rotation
        let world_from_camera = view.inverse();
        let (_, rot, _) = world_from_camera.to_scale_rotation_translation();

        self.rotation = rot;
    }

    /// Camera → world transform
    pub fn as_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position)
    }

    /// View matrix (world → camera)
    pub fn view_matrix(&self) -> Mat4 {
        self.as_matrix().inverse()
    }

    /// Camera's forward (−Z in right-handed systems)
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Camera's right (+X)
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Camera's up (+Y) – optional but convenient
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Transformation {
    pub transform: Mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Texture {
    pub img: ImageView,
    pub sampler: Option<Handle<Sampler>>,
}

pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
pub const LIGHT_TYPE_POINT: u32 = 1;
pub const LIGHT_TYPE_SPOT: u32 = 2;
pub const LIGHT_TYPE_AREA_RECT: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuLight {
    /// xyz = position (world space) for point/spot/area
    /// xyz = *unused* for directional
    /// w   = type (LIGHT_TYPE_*)
    pub position_type: Vec4,

    /// xyz = direction (world space, normalized) for directional/spot/area
    /// xyz = *unused* for point
    /// w   = range (point/spot) or max influence distance (area),
    ///       or 0.0 for “infinite” directional.
    pub direction_range: Vec4,

    /// rgb = color
    /// w   = intensity (luminous intensity / radiance scale)
    pub color_intensity: Vec4,

    /// x = inner cone cos(theta)   (for spot)
    /// y = outer cone cos(theta)   (for spot)
    /// z = area half-width         (for area rect)
    /// w = area half-height        (for area rect)
    /// or generally “misc packed params”
    pub spot_area: Vec4,

    /// x = flags (bitmask encoded as u32 bits in f32)
    /// y,z,w = padding / future use
    pub extra: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Material {
    pub base_color_texture_id: u16,
    pub normal_texture_id: u16,
    pub metallic_roughness_texture_id: u16,
    pub occlusion_texture_id: u16,
    pub emissive_texture_id: u16,
    pub _padding: u16,
}
