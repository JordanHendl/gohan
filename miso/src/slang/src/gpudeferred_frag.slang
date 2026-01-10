// GPU-driven deferred fragment shader.
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
    float4 position : SV_TARGET0;
    float4 albedo : SV_TARGET1;
    float4 normal : SV_TARGET2;
    float4 material_code : SV_TARGET3;
};

bool valid_texture(uint texture_id) {
  return texture_id != 0xffff;
}

[shader("fragment")]
FSOutput main(FSInput input) {
    FSOutput output;
    const Material material = meshi_bindless_materials[input.material_id];

    const bool use_vertex_color = (material.material_flags & MATERIAL_FLAG_VERTEX_COLOR) != 0;
    const bool use_emissive = (material.material_flags & MATERIAL_FLAG_EMISSIVE_ONLY) != 0;
    float4 base_color = input.color;
    if (!use_vertex_color) {
        const uint texture_id = use_emissive ? material.emissive_texture_id : material.base_color_texture_id;
        if (valid_texture(texture_id)) {
            base_color = sample_texture(texture_id, input.tex_coords);
        }
    }

    const float3 n = normalize(input.world_normal);
    const float3 t = normalize(input.world_tangent.xyz);
    const float3 b = normalize(cross(n, t)) * input.world_tangent.w;
    const float3 ortho_normal = normalize(cross(b, t));

    output.position = float4(input.world_pos, 1.0);
    output.albedo = base_color;
    output.normal = float4(ortho_normal, 1.0);
    output.material_code =
        float4(float(input.material_id), float(material.material_flags), 0.0, 1.0);
    return output;
}
