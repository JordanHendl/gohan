[[vk::binding(1, 0)]]
cbuffer Params
{
    uint offset;
};

[[vk::binding(0, 3)]]
RWStructuredBuffer<uint> outputData;

[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    outputData[id.x] = offset + id.x;
}
