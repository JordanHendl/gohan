import bindless;
import miso;

struct FSInput
{
    float2 tex_coords : TEXCOORD0;
};

struct FSOutput
{
    float4 color : SV_TARGET0;
};

cbuffer DeferredCombineInfoBuffer : register(b0, space2)
{
    DeferredCombineInfo deferred_combine_info;
};

[shader("fragment")]
FSOutput main(FSInput input)
{
    FSOutput output;
    uint position_id = deferred_combine_info.position_texture_id;
    uint albedo_id = deferred_combine_info.albedo_texture_id;
    uint normal_id = deferred_combine_info.normal_texture_id;
    uint material_id = deferred_combine_info.material_texture_id;

    float4 position =
        meshi_bindless_textures[position_id].Sample(meshi_bindless_samplers[position_id], input.tex_coords);
    float4 albedo =
        meshi_bindless_textures[albedo_id].Sample(meshi_bindless_samplers[albedo_id], input.tex_coords);
    float3 normal = normalize(
        meshi_bindless_textures[normal_id].Sample(meshi_bindless_samplers[normal_id], input.tex_coords).xyz);
    float material_code = meshi_bindless_textures[material_id]
                               .Sample(meshi_bindless_samplers[material_id], input.tex_coords)
                               .x;

    float depth_factor = saturate(1.0 / (1.0 + length(position.xyz)));
    float material_scale = 1.0 + (material_code * 0.001);
    float3 lighting = albedo.rgb * (0.2 + depth_factor);
    const Light light = meshi_bindless_lights[0];
    if (light.dir_light.valid()) {
        const DirectionalLight dir_light = light.dir_light.get();
        float3 light_dir = normalize(-dir_light.direction.xyz);
        float ndotl = max(dot(normal, light_dir), 0.0);
        lighting = albedo.rgb * dir_light.color.rgb * dir_light.intensity * ndotl * material_scale;
    }

    output.color = float4(lighting, 1.0);
    return output;
}
