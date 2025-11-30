import handle;
import lights;
struct Material {
  float4 base_color_factor;
  float4 emissive_factor;
  miso::Handle base_color;
  miso::Handle normal;
  miso::Handle emissive;
  miso::Handle pad0;
}

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



[[vk::binding(10)]] [[vk::combinedImageSampler]] uniform Texture2D bless_textures[];
[[vk::binding(10)]] [[vk::combinedImageSampler]] uniform SamplerState bless_samplers[];

[[vk::binding(11)]] uniform StructuredBuffer<miso::Light> bless_lights;


layout(std430)
[[vk::binding(20)]] uniform StructuredBuffer<PerObjectInfo> per_obj_ssbo;
static PerObjectInfo per_obj = per_obj_ssbo[0];

namespace miso {
float4 sample_texture(uint idx, float2 coords) {
  return bless_textures[idx].Sample(bless_samplers[idx], coords);  
}
}
