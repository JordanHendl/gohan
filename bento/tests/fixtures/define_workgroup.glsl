#version 450

#ifndef WORKGROUP_SIZE
#error "WORKGROUP_SIZE must be defined"
#endif

layout(local_size_x = WORKGROUP_SIZE, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer Output {
    uint data[];
} outputData;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    outputData.data[idx] = WORKGROUP_SIZE;
}
