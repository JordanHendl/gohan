import handle;
import lights;
struct Material {
  float4 base_color_factor;
  float4 emissive_factor;
  Handle base_color;
  Handle normal;
  Handle emissive;
  Handle pad0;
};

struct Camera {
  //todo

};

// Bindless textures (all available textures).
struct TextureSamplerPair {
    Texture2D t;
    SamplerState s;
};

// Dynamic per-frame data.
struct PerObjectInfo  {
    float4x4 transform;          // Model matrix.
    Handle scene_id;
    Handle material_id;
    Handle camera_id;
};

// BINDLESS VARIABLES
uniform Texture2D meshi_bindless_textures[] : register(t0, space0);
uniform SamplerState meshi_bindless_samplers[] : register(s0, space1);
uniform StructuredBuffer<Material> meshi_bindless_materials : register(t0, space2);
uniform StructuredBuffer<Camera> meshi_bindless_cameras : register(t0, space3);
uniform StructuredBuffer<Light> meshi_bindless_lights : register(t0, space4);

// PER OBJECT INFO
uniform StructuredBuffer<PerObjectInfo> per_obj_ssbo : register(t0, space5);

static PerObjectInfo per_obj = per_obj_ssbo[0];

// HELPERS
float4 sample_texture(uint idx, float2 coords) {
  return meshi_bindless_textures[idx].Sample(meshi_bindless_samplers[idx], coords);
}
