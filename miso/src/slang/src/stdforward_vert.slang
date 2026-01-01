// Forward Vertex Shader (stdforward_vert.slang)
import bindless_draw;
import bindless;
import miso;

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
    [[vk::location(0)]] float3 world_pos : POSITION_WS;
    [[vk::location(1)]] float3 world_normal : NORMAL_WS;
    [[vk::location(2)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float4 world_tangent : TANGENT0;
};

float4x4 view_from_camera(Camera camera) {
    return camera.world_from_camera;
}

float4x4 model_matrix() {
    // Prefer the shared transformation buffer, fallback to the per-object copy.
    if (per_obj.scene_id.valid()) {
        return meshi_bindless_transformations[per_obj.scene_id.id()].transform;
    }

    return per_obj.transform;
}

[shader("vertex")]
VSOutput main(VSInput input)
{
    VSOutput output;

    const Camera camera = meshi_bindless_cameras[per_obj.camera_id.id()];
    const float4x4 view = view_from_camera(camera);
    const float4x4 view_proj = mul(camera.projection, view);
    const float4x4 model = model_matrix();
    const float4x4 mvp = mul(view_proj, model);

    const float4 world_pos = mul(model, float4(input.position, 1.0));
    const float3 world_normal = normalize(mul(model, float4(input.normal, 0.0)).xyz);
    const float3 world_tangent = normalize(mul(model, float4(input.tangent.xyz, 0.0)).xyz);

    output.position = mul(mvp, float4(input.position, 1.0));
    output.world_pos = world_pos.xyz;
    output.world_normal = world_normal;
    output.tex_coords = input.tex_coords;
    output.color = input.color;
    output.world_tangent = float4(world_tangent, input.tangent.w);

    return output;
}
