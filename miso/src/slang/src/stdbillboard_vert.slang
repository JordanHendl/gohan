// Simple billboard vertex shader
import bindless_draw;
import bindless;

struct VSInput
{
    [[vk::location(0)]] float3 center : POSITION;
    [[vk::location(1)]] float2 offset : TEXCOORD0;
    [[vk::location(2)]] float2 size : TEXCOORD1;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float2 tex_coords : TEXCOORD2;
};

struct VSOutput
{
    float4 position : SV_POSITION;
    [[vk::location(0)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
};

[shader("vertex")]
VSOutput main(VSInput input)
{
    VSOutput output;

    const Camera camera = meshi_bindless_cameras[per_obj.camera_id.id()];
    const float4x4 view = camera.world_from_camera;

    const float4 view_center = mul(view, float4(input.center, 1.0));
    const float2 scaled_offset = input.offset * input.size;
    const float3 view_pos = view_center.xyz + float3(scaled_offset, 0.0);

    output.position = mul(camera.projection, float4(view_pos, 1.0));
    output.tex_coords = input.tex_coords;
    output.color = input.color;
    return output;
}
