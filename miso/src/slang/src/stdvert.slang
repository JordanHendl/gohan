// Vertex Shader (vertex.slang)
import bindless;
import miso;

struct VSInput
{
    float4 position;
    float4 normal;
    float4 color;
    float2 tex_coords;
    int4 joint_ids;
    float4 joints;
};

struct VSOutput
{
    float4 position : SV_POSITION;
    float3 color : COLOR;
    float3 pos;
    float3 normal;
    float2 tex_coords;
}

[shader("vertex")]
VSOutput main(VSInput input)
{
    VSOutput output;

    float4x4 t = mul(per_obj.transform, per_obj.viewproj);
    output.position = mul(float4(input.position.xyz, 1.0), t);
    output.pos = output.position.xyz;
    output.color = input.color.xyz;
    output.normal = (mul(input.normal, per_obj.transform)).xyz;
    output.tex_coords = input.tex_coords.xy;
    return output;
}

