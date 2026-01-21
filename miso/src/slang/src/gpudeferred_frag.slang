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

// Resolve the normal for the G-buffer, optionally sampling the normal map when PBR is enabled.
float3 resolve_normal(Material material, FSInput input) {
    float3 n = normalize(input.world_normal);
    const bool use_pbr_normal = (material.material_flags & MATERIAL_FLAG_PBR_NORMAL) != 0;
    if (!use_pbr_normal || !valid_texture(material.normal_texture_id)) {
        return n;
    }

    float3 t = normalize(input.world_tangent.xyz);
    float3 b = normalize(cross(n, t)) * input.world_tangent.w;
    float3x3 tbn = float3x3(t, b, n);
    float3 normal_map =
        sample_texture(material.normal_texture_id, input.tex_coords).xyz * 2.0 - 1.0;
    return normalize(mul(normal_map, tbn));
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

    // Store the resolved normal in the G-buffer for deferred lighting.
    const float3 ortho_normal = resolve_normal(material, input);
    // Default PBR values if no metallic-roughness texture is present.
    float metallic = 0.0;
    float roughness = 1.0;
    if (valid_texture(material.metallic_roughness_texture_id)) {
        const float4 metallic_roughness =
            sample_texture(material.metallic_roughness_texture_id, input.tex_coords);
        roughness = saturate(metallic_roughness.g);
        metallic = saturate(metallic_roughness.b);
    }

    output.position = float4(input.world_pos, 1.0);
    output.albedo = base_color;
    output.normal = float4(ortho_normal, 1.0);
    // Pack material id, flags, and PBR params for the combine pass.
    output.material_code =
        float4(float(input.material_id), float(material.material_flags), metallic, roughness);
    return output;
}
