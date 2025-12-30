import miso;
uniform StructuredBuffer<DeferredCombineInfo> per_obj_ssbo : register(t0, space1);
static DeferredCombineInfo per_obj = per_obj_ssbo[0];
