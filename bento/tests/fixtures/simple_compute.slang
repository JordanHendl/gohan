[[vk::binding(0, 0)]] RWStructuredBuffer<uint> data;

[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = 1;
}
