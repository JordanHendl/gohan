# Bindful reserved bindings example

This example demonstrates how the **bindful** (`DefaultState`) reservations work.
It compiles a simple shader that references the `meshi_timing` uniform buffer,
reflects the reserved binding with `Resolver`, builds a bind table via a
`RecipeBook`, and reads back the timing data that `furikake` writes every
update.

## Running

```bash
cargo run --example bindful_reserved
```

## What it shows

- Reflection confirms the shader exposes the reserved `meshi_timing` binding.
- The recipe book produces a bind table layout and bind table for that
  reservation automatically.
- After `DefaultState::update` the example maps the timing buffer and prints the
  current time and frame time values.
