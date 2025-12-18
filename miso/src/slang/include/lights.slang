struct DirectionalLight {
  float4 direction;
  float4 color;
  float intensity;
  float3 pad0;
};

struct OptionalDirectionalLight {
  DirectionalLight item;
  uint present;
  uint3 padding;

  bool valid() {
    return present != 0;
  }

  DirectionalLight get() {
    return item;
  }
};

struct Light {
  OptionalDirectionalLight dir_light;
};
