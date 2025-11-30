// Explicit register annotations to validate binding-name mapping
Texture2D<float4> colorTex : register(t0);
SamplerState linearSampler : register(s1);
RWStructuredBuffer<uint> outputData : register(u2);
cbuffer Params : register(b3)
{
    float4 tint;
};

[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    float4 color = colorTex.Sample(linearSampler, float2(0.0, 0.0)) + tint;
    outputData[id.x] = asuint(color.r);
}
