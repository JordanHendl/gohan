import handle;
import miso;
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
  column_major float4x4 world_from_camera;
  column_major float4x4 projection;
  float2 viewport;
  float near;
  float far;
  float fov_y_radians;
  uint projection_kind;
  float _padding;
};

struct Transformation {
  column_major float4x4 transform;
};

// BINDLESS VARIABLES (match furikake's reserved bindless state)
uniform Texture2D meshi_bindless_textures[] : register(t0, space0);
uniform Texture2D meshi_bindless_samplers[] : register(s0, space0);

uniform StructuredBuffer<Camera> meshi_bindless_cameras : register(t2, space0);
uniform StructuredBuffer<Transformation> meshi_bindless_transformations : register(t3, space0);
uniform StructuredBuffer<Material> meshi_bindless_materials : register(t4, space0);
uniform StructuredBuffer<Light> meshi_bindless_lights : register(t5, space0);

// HELPERS
float4 sample_texture(uint idx, float2 coords) {
  return meshi_bindless_textures[idx].Sample(meshi_bindless_samplers[idx], coords);
}
