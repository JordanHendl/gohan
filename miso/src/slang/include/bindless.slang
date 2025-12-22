import handle;
import lights;

struct Material {
  uint base_color_texture_id;
  uint normal_texture_id;
  uint metallic_roughness_texture_id;
  uint occlusion_texture_id;
  uint emissive_texture_id;
  uint render_mask;
  uint _padding;
};

struct Camera {
  float4x4 world_from_camera;
  float4x4 projection;
  float2 viewport;
  float near;
  float far;
  float fov_y_radians;
  uint projection_kind;
  float _padding;
};

struct Transformation {
  float4x4 transform;
};

// Dynamic per-frame data.
struct PerObjectInfo  {
    float4x4 transform;          // Model matrix fallback.
    Handle scene_id;
    Handle material_id;
    Handle camera_id;
};

// BINDLESS VARIABLES (match furikake's reserved bindless state)
uniform Texture2D meshi_bindless_textures[] : register(t0, space2);
uniform SamplerState meshi_bindless_samplers[] : register(s0, space7);
uniform StructuredBuffer<Camera> meshi_bindless_cameras : register(t0, space1);
uniform StructuredBuffer<Transformation> meshi_bindless_transformations : register(t0, space3);
uniform StructuredBuffer<Material> meshi_bindless_materials : register(t0, space4);
uniform StructuredBuffer<Light> meshi_bindless_lights : register(t0, space5);

// PER OBJECT INFO
uniform StructuredBuffer<PerObjectInfo> per_obj_ssbo : register(t0, space6);

static PerObjectInfo per_obj = per_obj_ssbo[0];

// HELPERS
float4 sample_texture(uint idx, float2 coords) {
  return meshi_bindless_textures[idx].Sample(meshi_bindless_samplers[idx], coords);
}
