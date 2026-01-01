// Sky Fragment Shader (stdsky_frag.slang)
import bindless;
import handle;

struct FSInput
{
    [[vk::location(0)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
};

struct FSOutput
{
    float4 color : SV_TARGET0;
};

struct SkyDrawInfo {
    column_major float4x4 transform;
    Handle camera_id;
    Handle material_id;
    float intensity;
    float horizon_falloff;
    float time;
    float _padding;
};

cbuffer SkyParams : register(b1, space1) {
    float3 horizon_tint;
    float intensity_scale;
    float3 zenith_tint;
    float _padding_params;
};

uniform StructuredBuffer<SkyDrawInfo> sky_draw_ssbo : register(t0, space1);
static SkyDrawInfo sky_draw = sky_draw_ssbo[0];

bool valid_texture(uint texture_id) {
  return texture_id != 0xffff;
}

[shader("fragment")]
FSOutput main(FSInput input)
{
    FSOutput output;
    const Material material = meshi_bindless_materials[sky_draw.material_id.id()];

    float4 base_color = input.color;
    if (valid_texture(material.base_color_texture_id)) {
        base_color = sample_texture(material.base_color_texture_id, input.tex_coords);
    }

    const float tint_blend = saturate(input.tex_coords.y * sky_draw.horizon_falloff);
    const float3 tint = lerp(horizon_tint, zenith_tint, tint_blend);
    const float intensity = sky_draw.intensity * intensity_scale;
    output.color = float4(base_color.rgb * tint * intensity, base_color.a);
    return output;
}
