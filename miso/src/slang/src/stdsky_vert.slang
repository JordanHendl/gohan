// Sky Vertex Shader (stdsky_vert.slang)
import bindless;
import handle;

struct VSInput
{
    [[vk::location(0)]] float3 position : POSITION;
    [[vk::location(1)]] float3 normal : NORMAL;
    [[vk::location(2)]] float4 tangent : TANGENT;
    [[vk::location(3)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(4)]] float4 color : COLOR;
};

struct VSOutput
{
    float4 position : SV_POSITION;
    [[vk::location(0)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
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

uniform StructuredBuffer<SkyDrawInfo> sky_draw_ssbo : register(t0, space1);
static SkyDrawInfo sky_draw = sky_draw_ssbo[0];

float4x4 view_from_camera(Camera camera) {
    return camera.world_from_camera;
}

[shader("vertex")]
VSOutput main(VSInput input)
{
    VSOutput output;

    const Camera camera = meshi_bindless_cameras[sky_draw.camera_id.id()];
    const float4x4 view = view_from_camera(camera);
    const float4x4 view_proj = mul(camera.projection, view);
    const float4x4 model = sky_draw.transform;
    const float4x4 mvp = mul(view_proj, model);

    output.position = mul(mvp, float4(input.position, 1.0));
    output.tex_coords = input.tex_coords;
    output.color = input.color;

    return output;
}
