import miso;
import bindless;
import bindless_deferred_combine;

struct FSInput {
  float2 tex_coords : TEXCOORD0;
};

struct FSOutput {
  float4 color : SV_TARGET0;
};

[shader("fragment")] FSOutput main(FSInput input) {
  FSOutput output;
  uint position_id = per_obj.position_texture_id;
  uint albedo_id = per_obj.albedo_texture_id;
  uint normal_id = per_obj.normal_texture_id;
  uint material_id = per_obj.material_texture_id;

  float4 position = meshi_bindless_textures[position_id].Sample(
      meshi_bindless_samplers[position_id], input.tex_coords);
  float4 albedo = meshi_bindless_textures[albedo_id].Sample(
      meshi_bindless_samplers[albedo_id], input.tex_coords);
  float3 normal = normalize(
      meshi_bindless_textures[normal_id]
          .Sample(meshi_bindless_samplers[normal_id], input.tex_coords)
          .xyz);
  float material_code =
      meshi_bindless_textures[material_id]
          .Sample(meshi_bindless_samplers[material_id], input.tex_coords)
          .x;

  float depth_factor = saturate(1.0 / (1.0 + length(position.xyz)));
  float material_scale = 1.0 + (material_code * 0.001);
  float3 lighting = albedo.rgb * (0.2 + depth_factor);
  const Light light = meshi_bindless_lights[0];
  if (light.valid()) {;
    float3 light_dir = normalize(-light.direction());
    float ndotl = max(dot(normal, light_dir), 0.0);
    lighting = albedo.rgb * light.color() * light.intensity() * ndotl *
               material_scale;
  }

  if (material_id != 1.0) {
    output.color = float4(lighting, 1.0);
  } else {
    // Choose a bias direction (Y = up is common)
    float shade = max(normal.y * 0.5 + 0.5, 0.2); // remap [-1,1] → [0,1]
    output.color = float4(albedo.rgb * float3(shade), 1.0);
  }
  
  output.color = float4(albedo.rgb, 1.0);
  return output;
}
