use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use dashi::{BufferView, Context, ContextInfo, ShaderType};
use furikake::recipe::RecipeBook;
use furikake::reservations::ReservedTiming;
use furikake::{DefaultState, Resolver};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct TimingData {
    current_time_ms: f32,
    frame_time_ms: f32,
}

fn compile_shader() -> bento::CompilationResult {
    let source = r#"
        #version 450 core
        layout(local_size_x = 1) in;

        layout(set = 0, binding = 0) uniform timing {
            float current_time_ms;
            float frame_time_ms;
        } meshi_timing;

        void main() {
            float jitter = sin(meshi_timing.current_time_ms * 0.001);
            if (jitter > 2.0) {
                // keep the compiler from stripping the uniform
                barrier();
            }
        }
    "#;

    let compiler = Compiler::new().expect("create bento compiler");
    compiler
        .compile(
            source.as_bytes(),
            &Request {
                name: Some("bindful_reserved_compute".to_string()),
                lang: ShaderLang::Glsl,
                stage: ShaderType::Compute,
                optimization: OptimizationLevel::None,
                debug_symbols: true,
            },
        )
        .expect("compile compute shader")
}

fn main() {
    let mut ctx = Context::headless(&ContextInfo::default()).expect("create dashi context");
    let mut state = DefaultState::new(&mut ctx);

    let shader = compile_shader();

    // Validate reserved bindings using the resolver.
    let resolver = Resolver::new(&state, &shader).expect("reflect reserved binding");
    println!(
        "Validated bindful reserved bindings: {:?}",
        resolver.resolved()
    );

    // Build a bind table from the reservation metadata.
    let book = RecipeBook::new(&mut ctx, &state, &[shader]).expect("build recipe book");
    let mut bt_recipes = book.recipes();
    println!("Created {} bind table recipe(s)", bt_recipes.len());

    let mut recipe = bt_recipes.pop().expect("timing bind table recipe");
    let _bind_table = recipe.cook(&mut ctx).expect("cook timing bind table");

    // Update the timing data and read the values written by the reservation.
    state.update().expect("refresh reserved timing");

    let timing = state
        .reserved::<ReservedTiming>("meshi_timing")
        .expect("access reserved timing");
    let mapped = ctx
        .map_buffer::<TimingData>(BufferView::new(timing.buffer()))
        .expect("map timing buffer");

    println!(
        "Timing snapshot -> current: {:.3}ms | frame: {:.3}ms",
        mapped[0].current_time_ms, mapped[0].frame_time_ms
    );

    ctx.unmap_buffer(timing.buffer())
        .expect("unmap timing buffer after read");
}
