# Bindless reserved bindings example

This example exercises every reserved item exposed by the **bindless**
(`BindlessState`) configuration:

- `meshi_timing` (uniform)
- `meshi_bindless_camera`
- `meshi_bindless_textures`
- `meshi_bindless_samplers`
- `meshi_bindless_transformations`
- `meshi_bindless_materials`

It compiles a compute shader that references all of these bindings, validates
that the shader matches the expected reservation metadata, mutates host-side
bindless data, and then builds the required bind tables from a
`RecipeBook`.

## Running

```bash
cargo run --example bindless_reserved
```

## What it shows

- Resolver validation across all six reserved resources.
- How to allocate, edit, and inspect bindless handles for cameras, textures,
  transformations, and materials.
- Automatic layout generation for a mixed bindful/bindless shader set via the
  recipe book helpers.

## Runtime mutation samples

The full example in `main.rs` already mutates every bindless reservation once.
Here are a few smaller snippets you can lift into your own runtime loops to
execute updates every frame.

### Animate cameras on the CPU and push them to the GPU

```rust
use furikake::reservations::bindless_camera::ReservedBindlessCamera;

let mut camera = None;
state
    .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
        camera = Some(cameras.add_camera());
    })
    .expect("allocate camera");

let camera = camera.expect("camera handle");

// In your frame loop.
state
    .reserved_mut::<ReservedBindlessCamera, _>("meshi_bindless_camera", |cameras| {
        let cam = cameras.camera_mut(camera);
        cam.position.x += 0.1; // slide along +X
    })
    .expect("animate camera");

// Flush the host-side changes into the GPU-visible buffer.
state.update().expect("upload bindless camera data");
```

### Swap a texture binding at runtime

```rust
use dashi::{Format, ImageInfo, ImageView, SamplerInfo};
use furikake::reservations::bindless_materials::ReservedBindlessMaterials;
use furikake::reservations::bindless_textures::ReservedBindlessTextures;

let image = ctx
    .make_image(&ImageInfo {
        debug_name: "bindless_albedo",
        dim: [1, 1, 1],
        format: Format::RGBA8,
        initial_data: Some(&[255, 255, 255, 255]),
        ..Default::default()
    })
    .expect("create albedo image");
let view = ImageView {
    img: image,
    ..Default::default()
};

let mut albedo = None;
state
    .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
        albedo = Some(textures.add_texture(view));
    })
    .expect("allocate texture slot");

// Or, provide an explicit sampler for the image view when reserving it.
// let sampler = ctx
//     .make_sampler(&SamplerInfo {
//         max_anisotropy: 4.0,
//         ..Default::default()
//     })
//     .expect("anisotropic sampler");
// state
//     .reserved_mut::<ReservedBindlessTextures, _>("meshi_bindless_textures", |textures| {
//         albedo = Some(textures.add_texture_with_sampler(view, Some(sampler)));
//     })
//     .expect("allocate texture slot with sampler");

let albedo = albedo.expect("texture handle");

// Replace the material binding to point at the newly streamed texture.
state
    .reserved_mut::<ReservedBindlessMaterials, _>("meshi_bindless_materials", |materials| {
        materials.material_mut(material_handle).base_color_texture_id = albedo;
    })
    .expect("swap bindless texture id");

state.update().expect("refresh bindless texture table");
```

### Build bind tables after runtime edits

Bind groups and bind tables can be built after you have finished mutating the
bindless data. This mirrors a frame where you write CPU-side structures first
and then issue GPU work.

```rust
use furikake::recipe::RecipeBook;

state.update().expect("flush bindless edits");

let book = RecipeBook::new(&mut ctx, &state, &[shader])
    .expect("generate layouts for bindless + bindful bindings");
let (mut bg_recipes, mut bt_recipes) = book.recipes();

for mut recipe in bg_recipes.drain(..) {
    let _bind_table = recipe.cook(&mut ctx).expect("cook bind table");
}
for mut recipe in bt_recipes.drain(..) {
    let _bind_table = recipe.cook(&mut ctx).expect("cook bind table");
}
```
