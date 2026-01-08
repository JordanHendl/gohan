// Vertex Shader (vertex.slang)
import bindless_draw;
import bindless;
import miso;

struct VSInput
{
    [[vk::location(0)]] float3 position : POSITION;
    [[vk::location(1)]] float3 normal : NORMAL;
    [[vk::location(2)]] float4 tangent : TANGENT;
    [[vk::location(3)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(4)]] float4 color : COLOR;
    [[vk::location(5)]] uint4 joints : JOINTS;
    [[vk::location(6)]] float4 weights : WEIGHTS;
};

struct VSOutput
{
    float4 position : SV_POSITION;
    [[vk::location(0)]] float3 world_pos : POSITION_WS;
    [[vk::location(1)]] float3 world_normal : NORMAL_WS;
    [[vk::location(2)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float4 world_tangent : TANGENT0;
};

float4x4 view_from_camera(Camera camera) {
    return camera.world_from_camera;
}

float4x4 model_matrix() {
    // Prefer the shared transformation buffer, fallback to the per-object copy.
    if (per_obj.scene_id.valid()) {
        return meshi_bindless_transformations[per_obj.scene_id.id()].transform;
    }

    return per_obj.transform;
}

float4x4 identity_matrix() {
    return float4x4(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0
    );
}

Handle skeleton_handle_from_state(AnimationState state) {
    Handle handle;
    handle.m_packed = state.skeleton_handle;
    return handle;
}

float4x4 joint_skin_matrix(uint joint_index) {
    if (per_obj.per_obj_joints_id.valid()) {
        const uint base = (uint)per_obj.per_obj_joints_id.id();
        return meshi_per_obj_joints[base + joint_index].transform;
    }

    const JointTransform joint = meshi_bindless_joints[joint_index];
    return mul(joint.bind_pose, joint.inverse_bind);
}

float4x4 skinning_matrix(uint4 joints, float4 weights) {
    const float weight_sum = weights.x + weights.y + weights.z + weights.w;
    if (weight_sum <= 0.0) {
        return identity_matrix();
    }

    Handle skeleton_handle = per_obj.skeleton_id;
    if (per_obj.animation_state_id.valid()) {
        const AnimationState state = meshi_bindless_skinning[per_obj.animation_state_id.id()];
        const Handle state_skeleton = skeleton_handle_from_state(state);
        if (state_skeleton.valid()) {
            skeleton_handle = state_skeleton;
        }
    }

    if (!skeleton_handle.valid()) {
        return identity_matrix();
    }

    const SkeletonHeader skeleton = meshi_bindless_skeletons[skeleton_handle.id()];
    const uint base = 0;
    float4x4 skin = float4x4(0.0);
    skin += weights.x * joint_skin_matrix(base + joints.x);
    skin += weights.y * joint_skin_matrix(base + joints.y);
    skin += weights.z * joint_skin_matrix(base + joints.z);
    skin += weights.w * joint_skin_matrix(base + joints.w);
    return skin;
}

[shader("vertex")]
VSOutput main(VSInput input)
{
    VSOutput output;

    const Camera camera = meshi_bindless_cameras[per_obj.camera_id.id()];
    const float4x4 view = view_from_camera(camera);
    const float4x4 view_proj = mul(camera.projection, view);
    const float4x4 model = model_matrix();
    const float4x4 skin = skinning_matrix(input.joints, input.weights);
    const float4x4 mvp = mul(view_proj, model);

    const float4 skinned_pos = mul(skin, float4(input.position, 1.0));
    const float3 skinned_normal = mul(skin, float4(input.normal, 0.0)).xyz;
    const float3 skinned_tangent = mul(skin, float4(input.tangent.xyz, 0.0)).xyz;

    const float4 world_pos = mul(model, skinned_pos);
    const float3 world_normal = normalize(mul(model, float4(skinned_normal, 0.0)).xyz);
    const float3 world_tangent = normalize(mul(model, float4(skinned_tangent, 0.0)).xyz);

    output.position = mul(mvp, skinned_pos);
    output.world_pos = world_pos.xyz;
    output.world_normal = world_normal;
    output.tex_coords = input.tex_coords;
    output.color = input.color;
    output.world_tangent = float4(world_tangent, input.tangent.w);

    return output;
}
