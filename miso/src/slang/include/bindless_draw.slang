import handle;

// Dynamic per-frame data.
struct PerObjectInfo  {
    column_major float4x4 transform;          // Model matrix fallback.
    Handle scene_id;
    Handle material_id;
    Handle camera_id;
};


uniform StructuredBuffer<PerObjectInfo> per_obj_ssbo : register(t0, space1);
static PerObjectInfo per_obj = per_obj_ssbo[0];
