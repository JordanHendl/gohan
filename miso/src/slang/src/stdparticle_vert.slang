// Particle billboard vertex shader
import bindless_draw;
import bindless;

struct VSInput
{
    [[vk::location(0)]] float2 offset : TEXCOORD0;
    [[vk::location(1)]] float2 tex_coords : TEXCOORD1;
};

struct VSOutput
{
    float4 position : SV_POSITION;
    [[vk::location(0)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
};

float2 rotate2d(float2 v, float angle)
{
    float s = sin(angle);
    float c = cos(angle);
    return float2(c * v.x - s * v.y, s * v.x + c * v.y);
}

[shader("vertex")]
VSOutput main(VSInput input, uint instance_id : SV_InstanceID)
{
    VSOutput output;
    const ParticleState particle = meshi_particles[instance_id];
    const Camera camera = meshi_bindless_cameras[per_obj.camera_id.id()];

    float4 view_center = float4(particle.position, 1.0);
    if (particle.space == PARTICLE_SPACE_WORLD) {
        view_center = mul(camera.world_from_camera, float4(particle.position, 1.0));
    }

    const float2 scaled_offset = input.offset * particle.size;
    const float2 rotated_offset = rotate2d(scaled_offset, particle.rotation);
    const float3 view_pos = view_center.xyz + float3(rotated_offset, 0.0);

    output.position = mul(camera.projection, float4(view_pos, 1.0));
    output.tex_coords = input.tex_coords;
    output.color = particle.color;
    return output;
}
