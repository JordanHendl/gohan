// Mixed explicit registers with an implicit binding to validate declaration-order fallback
Texture2D<float4> albedo;
SamplerState pointSampler : register(s3);
RWStructuredBuffer<uint> outputData : register(u1);
cbuffer FrameData : register(b2)
{
    float4 tint;
};

[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    float4 color = albedo.Sample(pointSampler, float2(0.0, 0.0)) + tint;
    outputData[id.x] = asuint(color.r);
}
