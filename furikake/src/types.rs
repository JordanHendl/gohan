use dashi::{Handle, ImageView, Sampler};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ProjectionKind {
    Perspective = 0,
    Orthographic = 1,
}

impl Default for ProjectionKind {
    fn default() -> Self {
        Self::Perspective
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Camera {
    /// Camera → world transform.
    pub world_from_camera: Mat4,
    /// Projection matrix (perspective or orthographic).
    pub projection: Mat4,
    /// Width/height of the render target this projection was built for.
    pub viewport: Vec2,
    pub near: f32,
    pub far: f32,
    /// Vertical field of view in radians when using perspective projection.
    pub fov_y_radians: f32,
    pub projection_kind: ProjectionKind,
    /// Padding to keep the struct 16-byte aligned when used in buffers.
    pub _padding: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(Vec3::ZERO, Quat::IDENTITY)
    }
}

impl Camera {
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        let mut camera = Self {
            world_from_camera: Mat4::from_rotation_translation(rotation, position),
            projection: Mat4::IDENTITY,
            viewport: Vec2::new(512.0, 512.0),
            near: 0.1,
            far: 1_000.0,
            fov_y_radians: std::f32::consts::FRAC_PI_4,
            projection_kind: ProjectionKind::Perspective,
            _padding: 0.0,
        };

        camera.update_projection();
        camera
    }

    /// Point the camera at a target world position.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let forward = (target - self.position()).normalize();

        // glam's look_to_rh gives a view matrix (world → camera)
        let view = Mat4::look_to_rh(self.position(), forward, up);

        self.world_from_camera = view.inverse();
    }

    /// Camera → world transform
    pub fn as_matrix(&self) -> Mat4 {
        self.world_from_camera
    }

    /// View matrix (world → camera)
    pub fn view_matrix(&self) -> Mat4 {
        self.world_from_camera.inverse()
    }

    /// Camera's forward (−Z in right-handed systems)
    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }

    /// Camera's right (+X)
    pub fn right(&self) -> Vec3 {
        self.rotation() * Vec3::X
    }

    /// Camera's up (+Y) – optional but convenient
    pub fn up(&self) -> Vec3 {
        self.rotation() * Vec3::Y
    }

    /// Extract the camera position from its world transform.
    pub fn position(&self) -> Vec3 {
        let (_, _, translation) = self.world_from_camera.to_scale_rotation_translation();
        translation
    }

    /// Extract the camera rotation from its world transform.
    pub fn rotation(&self) -> Quat {
        let (_, rotation, _) = self.world_from_camera.to_scale_rotation_translation();
        rotation
    }

    /// Set only the camera position while keeping its rotation.
    pub fn set_position(&mut self, position: Vec3) {
        let rotation = self.rotation();
        self.world_from_camera = Mat4::from_rotation_translation(rotation, position);
    }

    /// Set only the camera rotation while keeping its position.
    pub fn set_rotation(&mut self, rotation: Quat) {
        let position = self.position();
        self.world_from_camera = Mat4::from_rotation_translation(rotation, position);
    }

    /// Replace the camera → world matrix directly.
    pub fn set_transform(&mut self, world_from_camera: Mat4) {
        self.world_from_camera = world_from_camera;
    }

    /// Change the projection to a perspective matrix.
    pub fn set_perspective(
        &mut self,
        fov_y_radians: f32,
        width: f32,
        height: f32,
        near: f32,
        far: f32,
    ) {
        self.projection_kind = ProjectionKind::Perspective;
        self.viewport = Vec2::new(width, height);
        self.near = near;
        self.far = far;
        self.fov_y_radians = fov_y_radians;
        self.update_projection();
    }

    /// Update the orthographic projection using the given viewport dimensions.
    pub fn set_orthographic(&mut self, width: f32, height: f32, near: f32, far: f32) {
        self.projection_kind = ProjectionKind::Orthographic;
        self.viewport = Vec2::new(width, height);
        self.near = near;
        self.far = far;
        self.update_projection();
    }

    /// Update only the vertical field of view (perspective projections only).
    pub fn set_fov_y(&mut self, fov_y_radians: f32) {
        self.fov_y_radians = fov_y_radians;
        if matches!(self.projection_kind, ProjectionKind::Perspective) {
            self.update_projection();
        }
    }

    /// Adjust the viewport dimensions and regenerate the projection matrix.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport = Vec2::new(width, height);
        self.update_projection();
    }

    /// Update near/far clip planes while preserving the current projection mode.
    pub fn set_clip_planes(&mut self, near: f32, far: f32) {
        self.near = near;
        self.far = far;
        self.update_projection();
    }

    fn update_projection(&mut self) {
        match self.projection_kind {
            ProjectionKind::Perspective => {
                let aspect = if self.viewport.y.abs() <= f32::EPSILON {
                    1.0
                } else {
                    self.viewport.x / self.viewport.y
                };

                self.projection = Mat4::perspective_rh(
                    self.fov_y_radians,
                    aspect,
                    self.near.max(f32::EPSILON),
                    self.far,
                );
            }
            ProjectionKind::Orthographic => {
                let half_width = self.viewport.x * 0.5;
                let half_height = self.viewport.y * 0.5;
                self.projection = Mat4::orthographic_rh_gl(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    self.near,
                    self.far,
                );
            }
        }
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
pub struct Light {
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
    pub render_mask: u16,
    pub _padding: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matrix_close(a: Mat4, b: Mat4) {
        for (lhs, rhs) in a.to_cols_array().iter().zip(b.to_cols_array().iter()) {
            assert!((lhs - rhs).abs() < 1e-5, "matrix mismatch: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn defaults_to_perspective_projection() {
        let camera = Camera::default();
        let expected = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 1_000.0);

        assert_eq!(camera.viewport, Vec2::new(512.0, 512.0));
        assert!(matches!(
            camera.projection_kind,
            ProjectionKind::Perspective
        ));
        assert_matrix_close(camera.projection, expected);
    }

    #[test]
    fn updates_perspective_fov_and_viewport() {
        let mut camera = Camera::default();
        camera.set_viewport(1024.0, 512.0);
        camera.set_fov_y(std::f32::consts::FRAC_PI_3);

        let expected = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 2.0, 0.1, 1_000.0);

        assert_eq!(camera.viewport, Vec2::new(1024.0, 512.0));
        assert_matrix_close(camera.projection, expected);
    }

    #[test]
    fn switches_to_orthographic_projection() {
        let mut camera = Camera::default();
        camera.set_orthographic(200.0, 100.0, 0.5, 50.0);

        let expected = Mat4::orthographic_rh_gl(-100.0, 100.0, -50.0, 50.0, 0.5, 50.0);

        assert!(matches!(
            camera.projection_kind,
            ProjectionKind::Orthographic
        ));
        assert_eq!(camera.viewport, Vec2::new(200.0, 100.0));
        assert_eq!(camera.near, 0.5);
        assert_eq!(camera.far, 50.0);
        assert_matrix_close(camera.projection, expected);
    }
}
