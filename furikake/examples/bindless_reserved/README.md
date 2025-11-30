# Bindless reserved bindings example

This example exercises every reserved item exposed by the **bindless**
(`BindlessState`) configuration:

- `meshi_timing` (uniform)
- `meshi_bindless_camera`
- `meshi_bindless_textures`
- `meshi_bindless_transformations`
- `meshi_bindless_materials`

It compiles a compute shader that references all of these bindings, validates
that the shader matches the expected reservation metadata, mutates host-side
bindless data, and then builds the required bind group and bind tables from a
`RecipeBook`.

## Running

```bash
cargo run --example bindless_reserved
```

## What it shows

- Resolver validation across all five reserved resources.
- How to allocate, edit, and inspect bindless handles for cameras, textures,
  transformations, and materials.
- Automatic layout generation for a mixed bindful/bindless shader set via the
  recipe book helpers.
