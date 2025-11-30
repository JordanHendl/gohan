import option;

namespace miso {
  struct DirectionalLight {
    float4 direction;
    float4 color;
    float intensity;
    float3 pad0;
  };

  struct Light {
    Optional<DirectionalLight> dir_light;
  };
}
