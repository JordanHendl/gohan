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

// Bindless textures (all available textures).
struct TextureSamplerPair {
    Texture2D t;
    SamplerState s;
};

// Global settings (environment, rendering settings, etc.).
struct GlobalInfo {
  float app_time_ms;
};

// Per-frame data (camera, transformations, etc.).
struct PerFrameInfo {
  float4 camera_position;
};

// Dynamic per-frame data.
struct PerObjectInfo  {
    float4x4 transform;          // Model matrix.
    float4x4 viewproj;          // Model matrix.
    Material material;
};



uniform Texture2D bless_textures[] : register(t0, space0);
uniform SamplerState bless_samplers[] : register(s0, space1);

uniform StructuredBuffer<Light> bless_lights : register(t0, space2);


uniform StructuredBuffer<PerObjectInfo> per_obj_ssbo : register(t0, space3);
static PerObjectInfo per_obj = per_obj_ssbo[0];

float4 sample_texture(uint idx, float2 coords) {
  return bless_textures[idx].Sample(bless_samplers[idx], coords);
}
