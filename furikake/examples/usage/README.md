# Usage example

This example demonstrates how to reflect reserved bindings with `furikake`,
compile inline GLSL shaders, and render a simple quad using Dashi.

## Running

```bash
cargo run --example usage
```

## Expected output

The example validates the reserved `meshi_timing` binding for both shaders,
records a single draw, and prints confirmation messages similar to:

```
Validated reserved binding in vertex shader: [ResolveResult { name: "meshi_timing", exists: true, binding: BindGroupVariable { var_type: Uniform, binding: 0, count: 1 }, set: 0 }]
Validated reserved binding in fragment shader: [ResolveResult { name: "meshi_timing", exists: true, binding: BindGroupVariable { var_type: Uniform, binding: 0, count: 1 }, set: 0 }]
Rendered a quad with reserved timing binding!
```

The program draws offscreen; no window will appear.
