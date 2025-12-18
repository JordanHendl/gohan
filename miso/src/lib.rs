use bento::CompilationResult;

macro_rules! resolve_with_includes {
    ($a:expr, $b:expr) => {
        todo!("Implement macro to resolve at compile time and import all includes")
    };
}

pub fn stddeferred(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!("src/slang/src/stdvert.slang", "-Isrc/slang/include/");
    let fshader = resolve_with_includes!("src/slang/src/stdfrag.slang", "-Isrc/slang/include/");
    todo!("Compile vertex & fragment shader with includes")
}
