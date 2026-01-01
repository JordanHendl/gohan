// Ocean Fragment Shader (stdocean_frag.slang)
import bindless;
import handle;

struct FSInput
{
    [[vk::location(0)]] float3 world_pos : POSITION_WS;
    [[vk::location(1)]] float3 world_normal : NORMAL_WS;
    [[vk::location(2)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float4 world_tangent : TANGENT0;
};

struct FSOutput
{
    float4 color : SV_TARGET0;
};

struct OceanDrawInfo {
    column_major float4x4 transform;
    Handle camera_id;
    Handle material_id;
    float wave_scale;
    float wave_speed;
    float foam_intensity;
    float _padding;
};

cbuffer OceanParams : register(b1, space1) {
    float time_seconds;
    float2 wind_direction;
    float wave_frequency;
    float foam_threshold;
};

uniform StructuredBuffer<OceanDrawInfo> ocean_draw_ssbo : register(t0, space1);
static OceanDrawInfo ocean_draw = ocean_draw_ssbo[0];

bool valid_texture(uint texture_id) {
  return texture_id != 0xffff;
}

float3 resolve_normal(Material material, FSInput input) {
    float3 n = normalize(input.world_normal);
    if (!valid_texture(material.normal_texture_id)) {
        return n;
    }

    float3 t = normalize(input.world_tangent.xyz);
    float3 b = normalize(cross(n, t)) * input.world_tangent.w;
    float3x3 tbn = float3x3(t, b, n);
    float3 normal_map = sample_texture(material.normal_texture_id, input.tex_coords).xyz * 2.0 - 1.0;
    return normalize(mul(normal_map, tbn));
}

float3 camera_position(Camera camera) {
    return camera.world_from_camera[3].xyz;
}

[shader("fragment")]
FSOutput main(FSInput input)
{
    FSOutput output;
    const Material material = meshi_bindless_materials[ocean_draw.material_id.id()];
    const Camera camera = meshi_bindless_cameras[ocean_draw.camera_id.id()];

    float4 base_color = input.color;
    if (valid_texture(material.base_color_texture_id)) {
        base_color = sample_texture(material.base_color_texture_id, input.tex_coords);
    }

    const float3 normal = resolve_normal(material, input);
    const float2 wave_uv =
        input.tex_coords * wave_frequency + wind_direction * (time_seconds * ocean_draw.wave_speed);
    const float wave = sin(wave_uv.x + wave_uv.y) * ocean_draw.wave_scale;
    float3 lighting = base_color.rgb * (0.08 + wave * 0.02);
    const Light light = meshi_bindless_lights[0];
    if (light.valid()) {
        float3 light_dir = normalize(-light.direction());
        float ndotl = max(dot(normal, light_dir), 0.0);
        lighting += base_color.rgb * light.color() * light.intensity() * ndotl;
    }

    const float3 view_dir = normalize(camera_position(camera) - input.world_pos);
    const float fresnel = pow(1.0 - saturate(dot(normal, view_dir)), 5.0);
    const float foam = saturate((fresnel - foam_threshold) * ocean_draw.foam_intensity);
    const float3 highlight = lerp(lighting, float3(1.0, 1.0, 1.0), fresnel * 0.5 + foam);

    output.color = float4(highlight, base_color.a);
    return output;
}
