//scene_cull.comp.glsl
#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

struct SceneObject {
    mat4 local_transform;
    mat4 world_transform;
    uint scene_mask;
    uint parent_slot;
    uint dirty;
    uint is_active;
    uint parent;
    uint child_count;
    uint children[16];
};

struct SceneBin {
    uint id;
    uint mask;
};

struct CulledObject {
    mat4 total_transform;
    uint bin_id;
};

struct Camera {
    mat4 world_from_camera;
    mat4 projection;
    vec2 viewport;
    float near;
    float far;
    float fov_y_radians;
    uint projection_kind;
    float _padding;
};

layout(set = 0, binding = 0) buffer SceneObjects {
    SceneObject objects[];
} objects;

layout(set = 0, binding = 1) buffer SceneBins {
    SceneBin bins[];
} bins;

layout(set = 0, binding = 2) buffer CulledBins {
    CulledObject culled[];
} culled;

layout(set = 0, binding = 3) buffer BinCounts {
    uint counts[];
} counts;

layout(set = 0, binding = 4) uniform SceneParams {
    uint num_bins;
    uint max_objects;
    uint _padding1;
} params;

layout(set = 0, binding = 5) uniform SceneCamera {
    uint slot;
} camera;

layout(set = 1, binding = 0) buffer Cameras {
    Camera cameras[];
} cameras;

vec3 camera_position(const Camera cam) {
    return cam.world_from_camera[3].xyz;
}

vec3 camera_forward(const Camera cam) {
    return -cam.world_from_camera[2].xyz;
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= params.max_objects) {
        return;
    }

    SceneObject obj = objects.objects[idx];
    if (obj.is_active == 0) {
        return;
    }

    if (camera.slot == 0xffffffffu) {
        return;
    }

    Camera cam = cameras.cameras[camera.slot];
    vec3 world_position = obj.world_transform[3].xyz;
    vec3 to_object = world_position - camera_position(cam);
    vec3 forward = normalize(camera_forward(cam));

    if (dot(forward, to_object) <= 0.0) {
        return;
    }

    for (uint bin = 0; bin < params.num_bins; ++bin) {
        if ((obj.scene_mask & bins.bins[bin].mask) == 0) {
            continue;
        }

        uint write_index = atomicAdd(counts.counts[bin], 1);
        if (write_index >= params.max_objects) {
            continue;
        }

        uint target = bin * params.max_objects + write_index;
        culled.culled[target].total_transform = obj.world_transform;
        culled.culled[target].bin_id = bins.bins[bin].id;
    }
}
