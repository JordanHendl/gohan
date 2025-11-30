// Fragment Shader (fragment.slang)
import bindless;
import miso;

struct FSInput
{
    float3 color : COLOR;
    float3 position;
    float3 normal;
    float2 tex_coords;
};

struct FSOutput
{
    float4 color : SV_TARGET;
};

bool valid_color(float4 color) {
  return color.x != 0.0 && color.y != 0.0 && color.z != 0.0 && color.w != 0.0;
}

float4 process_lights(const FSInput i, float4 in_color) {
  float3 color = in_color.xyz;
  for(uint idx = 0; idx < 1024; idx++) {
    let light = bless_lights[idx];
    
    if(light.dir_light.valid()) {
       float3 norm = i.normal;
       float3 light_dir = light.dir_light.get().direction.xyz;
       float3 diff = max(dot(-light_dir, norm), 0.0);
       color = color.xyz * diff;
    }
  }

  return float4(color, in_color.a);
}

[shader("fragment")]
FSOutput main(FSInput input)
{
    FSOutput output;
    if(per_obj.material.base_color.valid()) {
      output.color = miso::sample_texture(per_obj.material.base_color.id(), input.tex_coords); 
    } else if(valid_color(per_obj.material.base_color_factor)) {
      output.color = float4(per_obj.material.base_color_factor);
    } else {
      output.color = float4(input.color, 1.0); // Output the interpolated vertex color
    }

    output.color = process_lights(input, output.color);

    if(per_obj.material.emissive.valid()) {
      output.color *= miso::sample_texture(per_obj.material.emissive.id(), input.tex_coords); 
    } else if(valid_color(per_obj.material.emissive_factor)) {
      output.color *= float4(per_obj.material.emissive_factor);
    }

    return output;
}

