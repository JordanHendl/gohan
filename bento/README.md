[BENTO]

Bento packages shaders and their associated metadata into the Bento Format, a compact binary layout designed around SPIR-V output. Each compiled artifact is a Bento File that captures the shader stage, bind group variables, and the generated SPIR-V payload so callers can load or inspect the output without re-running the compiler.

## Project overview

* **Supported sources**: GLSL, HLSL, and Slang, reflected through shaderc and rspirv-reflect.
* **Artifacts**: Bento Files contain the serialized `CompilationResult`, including reflection data and SPIR-V words.
* **Tooling**: Two binaries ship with the crate:
  * `bentosc` compiles shaders into the Bento Format.
  * `bentoinspect` reads an existing Bento File and emits a summary or pretty JSON.

## CLI usage

The `bentosc` binary compiles shader sources into serialized Bento Files. Typical usage:

```
bentosc tests/fixtures/simple_compute.glsl \
    --stage compute \
    --lang glsl \
    --opt performance \
    --output target/simple_compute.bto \
    --name simple_compute \
    --verbose
```

Supported languages include GLSL, HLSL, and Slang via the `--lang` flag.

The command prints metadata about the compiled shader when `--verbose` is provided and writes the Bento File to the path specified by `--output`.
