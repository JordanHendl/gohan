// GPU-driven forward fragment shader.
import bindless_draw_gpu;
import bindless;
import miso;

struct FSInput {
    [[vk::location(0)]] float3 world_pos : POSITION_WS;
    [[vk::location(1)]] float3 world_normal : NORMAL_WS;
    [[vk::location(2)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float4 world_tangent : TANGENT0;
    [[vk::location(5)]] nointerpolation uint material_id : MATERIAL_ID;
};

struct FSOutput {
    float4 color : SV_TARGET0;
};

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

[shader("fragment")]
FSOutput main(FSInput input) {
    FSOutput output;
    const Material material = meshi_bindless_materials[input.material_id];

    const bool use_vertex_color = (material.material_flags & MATERIAL_FLAG_VERTEX_COLOR) != 0;
    const bool is_unlit = (material.material_flags & MATERIAL_FLAG_UNLIT) != 0;
    const bool is_toon = (material.material_flags & MATERIAL_FLAG_TOON) != 0;

    float4 base_color = input.color;
    if (!use_vertex_color && valid_texture(material.base_color_texture_id)) {
        base_color = sample_texture(material.base_color_texture_id, input.tex_coords);
    }

    if (is_unlit) {
        output.color = base_color;
        return output;
    }

    const float3 normal = resolve_normal(material, input);
    float3 lighting = base_color.rgb * 0.2;
    const Light light = meshi_bindless_lights[0];
    if (light.valid()) {
        float3 light_dir = normalize(-light.direction());
        float ndotl = max(dot(normal, light_dir), 0.0);
        if (is_toon) {
            const float toon_steps = 4.0;
            ndotl = floor(ndotl * toon_steps) / toon_steps;
        }
        lighting += base_color.rgb * light.color() * light.intensity() * ndotl;
    }

    output.color = float4(lighting, base_color.a);
    return output;
}
