import handle;

// Dynamic per-frame data for GPU-driven draws.
struct PerDrawData {
  Handle scene_id;
  Handle transform_id;
  Handle material_id;
  Handle skeleton_id;
  Handle animation_state_id;
  Handle per_obj_joints_id;
  uint vertex_id;
  uint vertex_count;
  uint index_id;
  uint index_count;
  uint terrain_height_texture_id;
  uint terrain_normal_texture_id;
  uint terrain_blend_texture_id;
  uint terrain_blend_ids_texture_id;
  uint terrain_clipmap_tile;
};

uniform StructuredBuffer<PerDrawData> per_draw_ssbo : register(t0, space1);

// Dynamic per-frame data for GPU-driven draws.
struct PerSceneData {
  Handle camera;
};

uniform StructuredBuffer<PerSceneData> per_scene_ssbo : register(t0, space2);
static PerSceneData scene = per_scene_ssbo[0];

