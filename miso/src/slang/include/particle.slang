static const uint PARTICLE_SPACE_WORLD = 0;
static const uint PARTICLE_SPACE_SCREEN = 1;

struct ParticleState {
  float3 position;
  float lifetime;
  float3 velocity;
  float age;
  float4 color;
  float2 size;
  float rotation;
  uint space;
  uint frame_index;
  uint flags;
  float2 _padding;
};

struct ParticleEmitterState {
  uint first_particle;
  uint max_particles;
  uint live_count;
  uint flags;
  float spawn_rate;
  float spawn_accumulator;
  uint seed;
  uint space;
  float3 position;
  float _padding;
  float3 direction;
  float spread;
  float4 start_color;
  float4 end_color;
  float2 start_size;
  float2 end_size;
  float2 lifetime_range;
  float2 speed_range;
  float2 rotation_range;
};
