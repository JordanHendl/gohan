import handle;
import miso;
import lights;
import particle;

struct Material {
  uint base_color_texture_id;
  uint normal_texture_id;
  uint metallic_roughness_texture_id;
  uint occlusion_texture_id;
  uint emissive_texture_id;
  uint material_flags;
  uint render_mask;
  uint _padding;
};

struct Camera {
  column_major float4x4 world_from_camera;
  column_major float4x4 projection;
  float2 viewport;
  float near;
  float far;
  float fov_y_radians;
  uint projection_kind;
  float _padding;
};

struct Transformation {
  column_major float4x4 transform;
};

struct SkeletonHeader {
  uint joint_count;
  uint joint_offset;
  uint bind_pose_offset;
  uint _padding;
};

struct JointTransform {
  int parent_index;
    uint _padding0;
    uint _padding1;
    uint _padding2;
  column_major float4x4 bind_pose;
  column_major float4x4 inverse_bind;
};

struct AnimationClip {
  float duration;
  uint track_count;
  uint track_offset;
  uint _padding;
};

struct AnimationTrack {
  uint joint_index;
  uint keyframe_count;
  uint keyframe_offset;
  uint _padding;
};

struct AnimationKeyframe {
  float time;
  float _padding0;
  float _padding1;
  float _padding2;
  float4 value;
};

struct AnimationState {
  uint clip_handle;
  uint skeleton_handle;
  float start_time;
  float playback_rate;
  uint looping;
  uint _padding1;
  uint _padding2;
  uint _padding3;
};

struct PerObjectJointTransform {
  column_major float4x4 transform;
};

// BINDLESS VARIABLES (match furikake's reserved bindless state)
uniform Texture2D meshi_bindless_textures[] : register(t0, space0);
uniform SamplerState meshi_bindless_samplers[] : register(s1, space0);

uniform StructuredBuffer<Camera> meshi_bindless_cameras : register(t2, space0);
uniform StructuredBuffer<Transformation> meshi_bindless_transformations : register(t3, space0);
uniform StructuredBuffer<Material> meshi_bindless_materials : register(t4, space0);
uniform StructuredBuffer<Light> meshi_bindless_lights : register(t5, space0);
uniform StructuredBuffer<SkeletonHeader> meshi_bindless_skeletons : register(t6, space0);
uniform StructuredBuffer<JointTransform> meshi_bindless_joints : register(t7, space0);
uniform StructuredBuffer<AnimationClip> meshi_bindless_animations : register(t8, space0);
uniform StructuredBuffer<AnimationTrack> meshi_bindless_animation_tracks : register(t9, space0);
uniform StructuredBuffer<AnimationKeyframe> meshi_bindless_animation_keyframes : register(t10, space0);
uniform StructuredBuffer<AnimationState> meshi_bindless_skinning : register(t11, space0);
uniform ByteAddressBuffer meshi_bindless_vertices[] : register(t12, space0);
uniform StructuredBuffer<uint> meshi_bindless_indices : register(t13, space0);
uniform StructuredBuffer<ParticleState> meshi_particles : register(t14, space0);
uniform StructuredBuffer<PerObjectJointTransform> meshi_per_obj_joints : register(t15, space0);

// HELPERS
float4 sample_texture(uint idx, float2 coords) {
  return meshi_bindless_textures[idx].Sample(meshi_bindless_samplers[idx], coords);
}
