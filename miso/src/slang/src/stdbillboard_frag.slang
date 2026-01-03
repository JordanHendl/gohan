// Simple billboard fragment shader
import bindless_draw;
import bindless;

struct FSInput
{
    [[vk::location(0)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
};

struct FSOutput
{
    float4 color : SV_TARGET0;
};

bool valid_texture(uint texture_id) {
  return texture_id != 0xffff;
}

[shader("fragment")]
FSOutput main(FSInput input)
{
    FSOutput output;
    const Material material = meshi_bindless_materials[per_obj.material_id.id()];

    float4 base_color = input.color;
    if (valid_texture(material.base_color_texture_id)) {
        base_color = sample_texture(material.base_color_texture_id, input.tex_coords) * input.color;
    }

    output.color = base_color;
    return output;
}
