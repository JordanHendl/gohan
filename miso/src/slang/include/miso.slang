struct DeferredCombineInfo {
  uint position_texture_id;
  uint albedo_texture_id;
  uint normal_texture_id;
  uint material_texture_id;
};

static const uint MATERIAL_FLAG_PBR_NORMAL = 1u;
static const uint MATERIAL_FLAG_VERTEX_COLOR = 1u << 1;
static const uint MATERIAL_FLAG_UNLIT = 1u << 2;
static const uint MATERIAL_FLAG_TOON = 1u << 3;
static const uint MATERIAL_FLAG_EMISSIVE_ONLY = 1u << 4;
