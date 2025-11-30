// Explicit register annotations to validate binding-name mapping for Slang
Texture2D<float4> colorTex : register(t1);
SamplerState linearSampler : register(s0);
RWStructuredBuffer<uint> outputData : register(u3);
cbuffer Params : register(b2)
{
    float4 tint;
};

[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    float4 color = colorTex.Sample(linearSampler, float2(0.0, 0.0)) + tint;
    outputData[id.x] = asuint(color.g);
}
