
const uint LIGHT_TYPE_DIRECTIONAL = 0;
const uint LIGHT_TYPE_POINT = 1;
const uint LIGHT_TYPE_SPOT = 2;
const uint LIGHT_TYPE_AREA_RECT = 3;

struct Light {
  /// xyz = position (world space) for point/spot/area
  /// xyz = *unused* for directional
  /// w   = type (LIGHT_TYPE_*)
  float4 position_type;

  /// xyz = direction (world space, normalized) for directional/spot/area
  /// xyz = *unused* for point
  /// w   = range (point/spot) or max influence distance (area),
  ///       or 0.0 for “infinite” directional.
  float4 direction_range;

  /// rgb = color
  /// w   = intensity (luminous intensity / radiance scale)
  float4 color_intensity;

  /// x = inner cone cos(theta)   (for spot)
  /// y = outer cone cos(theta)   (for spot)
  /// z = area half-width         (for area rect)
  /// w = area half-height        (for area rect)
  /// or generally “misc packed params”
  float4 spot_area;

  /// x = flags (bitmask encoded as u32 bits in f32)
  /// y,z,w = padding / future use
  float4 extra;

  bool valid() {
      return (int)position_type.w >= 0;
  }
  
  float3 direction() {
    return direction_range.rgb;
  }

  float range() {
    return direction_range.w;
  }

  float3 color() {
    return color_intensity.rgb;
  }

  float intensity() {
    return color_intensity.w;
  }

};
