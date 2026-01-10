// GPU-driven deferred vertex shader.
import bindless_draw_gpu;
import bindless;
import miso;

struct VSInput {
    uint vertex_id : SV_VertexID;
    uint instance_id : SV_InstanceID;
};

struct VSOutput {
    float4 position : SV_POSITION;
    [[vk::location(0)]] float3 world_pos : POSITION_WS;
    [[vk::location(1)]] float3 world_normal : NORMAL_WS;
    [[vk::location(2)]] float2 tex_coords : TEXCOORD0;
    [[vk::location(3)]] float4 color : COLOR0;
    [[vk::location(4)]] float4 world_tangent : TANGENT0;
    [[vk::location(5)]] nointerpolation uint material_id : MATERIAL_ID;
};

struct VertexData {
    float3 position;
    float3 normal;
    float4 tangent;
    float2 tex_coords;
    float4 color;
    uint4 joints;
    float4 weights;
};

static const uint VERTEX_STRIDE = 96u;

float4x4 view_from_camera(Camera camera) {
    return camera.world_from_camera;
}

float4x4 identity_matrix() {
    return float4x4(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0
    );
}

float4x4 model_matrix(PerDrawData draw) {
    if (draw.transform_id.valid()) {
        return meshi_bindless_transformations[draw.transform_id.id()].transform;
    }

    if (draw.scene_id.valid()) {
        return meshi_bindless_transformations[draw.scene_id.id()].transform;
    }

    return identity_matrix();
}

float4x4 joint_skin_matrix(PerDrawData draw, uint joint_index) {
    const uint base = (uint)draw.per_obj_joints_id.id();
    return meshi_per_obj_joints[base + joint_index].transform;
}

float4x4 skinning_matrix(PerDrawData draw, uint4 joints, float4 weights) {
    if (!draw.per_obj_joints_id.valid()) {
        return identity_matrix();
    }

    const float weight_sum = weights.x + weights.y + weights.z + weights.w;
    if (weight_sum <= 0.0) {
        return identity_matrix();
    }

    float4x4 skin = float4x4(0.0);
    skin += weights.x * joint_skin_matrix(draw, joints.x);
    skin += weights.y * joint_skin_matrix(draw, joints.y);
    skin += weights.z * joint_skin_matrix(draw, joints.z);
    skin += weights.w * joint_skin_matrix(draw, joints.w);
    return skin;
}

VertexData load_vertex_data(PerDrawData draw, uint vertex_id) {
    VertexData data;
    #define vertex_buffer meshi_bindless_vertices

    // draw.vertex_id =  offset to start of mesh vertices
    // vertex_id/input.vertex_id = current vertex id of draw
    const uint base = draw.vertex_id * vertex_id * VERTEX_STRIDE;

    data.position = asfloat(vertex_buffer.Load3(base + 0));
    data.normal = asfloat(vertex_buffer.Load3(base + 12));
    data.tangent = asfloat(vertex_buffer.Load4(base + 24));
    data.tex_coords = asfloat(vertex_buffer.Load2(base + 40));
    data.color = asfloat(vertex_buffer.Load4(base + 48));
    data.joints = vertex_buffer.Load4(base + 64);
    data.weights = asfloat(vertex_buffer.Load4(base + 80));

    return data;
}

[shader("vertex")]
VSOutput main(VSInput input) {
    VSOutput output;
    const PerDrawData draw = per_draw_ssbo[input.instance_id];
    const VertexData vertex = load_vertex_data(draw, input.vertex_id);

    const Camera camera = meshi_bindless_cameras[draw.scene_id.id()];
    const float4x4 view = view_from_camera(camera);
    const float4x4 view_proj = mul(camera.projection, view);
    const float4x4 model = model_matrix(draw);
    const float4x4 skin = skinning_matrix(draw, vertex.joints, vertex.weights);
    const float4x4 mvp = mul(view_proj, model);

    const float4 skinned_pos = mul(skin, float4(vertex.position, 1.0));
    const float3 skinned_normal = mul(skin, float4(vertex.normal, 0.0)).xyz;
    const float3 skinned_tangent = mul(skin, float4(vertex.tangent.xyz, 0.0)).xyz;

    const float4 world_pos = mul(model, skinned_pos);
    const float3 world_normal = normalize(mul(model, float4(skinned_normal, 0.0)).xyz);
    const float3 world_tangent = normalize(mul(model, float4(skinned_tangent, 0.0)).xyz);

    output.position = mul(mvp, skinned_pos);
    output.world_pos = world_pos.xyz;
    output.world_normal = world_normal;
    output.tex_coords = vertex.tex_coords;
    output.color = vertex.color;
    output.world_tangent = float4(world_tangent, vertex.tangent.w);
    output.material_id = (uint)draw.material_id.id();

    output.position.y = -output.position.y;
    return output;
}
