struct DeferredCombineInfo {
  uint position_texture_id;
  uint albedo_texture_id;
  uint normal_texture_id;
  uint material_texture_id;
};

static const uint MATERIAL_FLAG_VERTEX_COLOR = 1u;
static const uint MATERIAL_FLAG_UNLIT = 1u << 1;
static const uint MATERIAL_FLAG_TOON = 1u << 2;
