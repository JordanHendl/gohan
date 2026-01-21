import miso;
import bindless_deferred_combine;

struct FSInput {
  float4 svpos : SV_Position;
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

  float4 position = sample_texture(position_id, int2(input.svpos.xy));
  float4 albedo = sample_texture(albedo_id, int2(input.svpos.xy));
  float4 normal = sample_texture(normal_id, int2(input.svpos.xy));
  float4 material_code = sample_texture(material_id, int2(input.svpos.xy));

  // Decode material flags and packed PBR parameters from the G-buffer.
  float depth_factor = saturate(1.0 / (1.0 + length(position.xyz)));
  uint material_flags = uint(material_code.y + 0.5);
  bool is_unlit = (material_flags & MATERIAL_FLAG_UNLIT) != 0;
  bool is_toon = (material_flags & MATERIAL_FLAG_TOON) != 0;
  bool is_emissive_only = (material_flags & MATERIAL_FLAG_EMISSIVE_ONLY) != 0;
  float metallic = saturate(material_code.z);
  float roughness = saturate(material_code.w);
  float3 n = normalize(normal.xyz);
  // Start with a small ambient term to avoid totally black output.
  float3 lighting = albedo.rgb * (0.02 + depth_factor * 0.1);

  if (is_unlit) {
    output.color = float4(albedo.rgb, 1.0);
    return output;
  }

  if (is_emissive_only) {
    output.color = float4(albedo.rgb, 1.0);
    return output;
  }

  // Build view-dependent data needed for the Cook-Torrance BRDF.
  const Camera camera = meshi_bindless_cameras[0];
  float3 camera_pos = camera.world_from_camera[3].xyz;
  float3 view_dir = normalize(camera_pos - position.xyz);
  float3 f0 = lerp(float3(0.04, 0.04, 0.04), albedo.rgb, metallic);
  float n_dot_v = max(dot(n, view_dir), 0.0);

  // Iterate all bindless lights and accumulate contributions.
  uint light_count = 0;
  meshi_bindless_lights.GetDimensions(light_count);
  for (uint light_index = 0; light_index < light_count; ++light_index) {
    const Light light = meshi_bindless_lights[light_index];
    if (!light.valid()) {
      continue;
    }

    // Resolve light direction and attenuation per light type.
    uint light_type = uint(light.position_type.w + 0.5);
    float3 light_dir = float3(0.0, 0.0, 0.0);
    float attenuation = 1.0;
    if (light_type == LIGHT_TYPE_DIRECTIONAL) {
      light_dir = normalize(-light.direction());
    } else {
      float3 light_vec = light.position_type.xyz - position.xyz;
      float distance = length(light_vec);
      if (distance > 0.0) {
        light_dir = light_vec / distance;
      }
      float range = light.range();
      if (range > 0.0) {
        float falloff = saturate(1.0 - (distance / range));
        attenuation = falloff * falloff;
      }
    if (light_type == LIGHT_TYPE_SPOT) {
      float3 spot_dir = normalize(light.direction());
      float cos_theta = dot(-light_dir, spot_dir);
      float inner = light.spot_area.x;
      float outer = light.spot_area.y;
      float spot = saturate((cos_theta - outer) / max(inner - outer, 0.001));
      attenuation *= spot;
    }
    }

    float n_dot_l = max(dot(n, light_dir), 0.0);
    if (n_dot_l <= 0.0) {
      continue;
    }
    if (is_toon) {
      const float toon_steps = 4.0;
      n_dot_l = floor(n_dot_l * toon_steps) / toon_steps;
    }

    // Cook-Torrance shading with GGX distribution, Schlick Fresnel, and Smith masking.
    float3 half_vec = normalize(view_dir + light_dir);
    float n_dot_h = max(dot(n, half_vec), 0.0);
    float v_dot_h = max(dot(view_dir, half_vec), 0.0);

    float alpha = max(roughness * roughness, 0.001);
    float alpha2 = alpha * alpha;
    float denom = (n_dot_h * n_dot_h) * (alpha2 - 1.0) + 1.0;
    float d = alpha2 / max(3.14159265 * denom * denom, 0.001);

    float k = (roughness + 1.0);
    k = (k * k) / 8.0;
    float g_v = n_dot_v / max(n_dot_v * (1.0 - k) + k, 0.001);
    float g_l = n_dot_l / max(n_dot_l * (1.0 - k) + k, 0.001);
    float g = g_v * g_l;

    float3 f = f0 + (1.0 - f0) * pow(1.0 - v_dot_h, 5.0);
    float3 specular = (d * g) * f / max(4.0 * n_dot_v * n_dot_l, 0.001);
    float3 kd = (1.0 - f) * (1.0 - metallic);
    float3 diffuse = kd * albedo.rgb / 3.14159265;

    float3 light_color = light.color() * light.intensity();
    lighting += (diffuse + specular) * light_color * n_dot_l * attenuation;
  }

  output.color = float4(lighting, 1.0);
  return output;
}
