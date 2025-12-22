use bento::{InterfaceVariable, ShaderVariable, VertexLayout};
use miso::stddeferred;

fn print_interface(label: &str, variables: &[InterfaceVariable]) {
    println!("{label}:");
    if variables.is_empty() {
        println!("  (none)");
        return;
    }

    for var in variables {
        println!(
            "  - {} @ location {:?} format {:?}",
            var.name, var.location, var.format
        );
    }
}

fn print_bindings(bindings: &[ShaderVariable]) {
    println!("Bindings:");
    if bindings.is_empty() {
        println!("  (none)");
        return;
    }

    for var in bindings {
        println!(
            "  - {} (set {}, binding {}, count {}, type {:?})",
            var.name, var.set, var.kind.binding, var.kind.count, var.kind.var_type
        );
    }
}

fn print_vertex_layout(layout: &VertexLayout) {
    println!(
        "Vertex layout: stride {}, rate {:?}",
        layout.stride, layout.rate
    );

    for entry in &layout.entries {
        println!(
            "  - location {} offset {} format {:?}",
            entry.location, entry.offset, entry.format
        );
    }
}

fn main() {
    let defines: Vec<String> = std::env::args().skip(1).collect();
    let shaders = stddeferred(&defines);

    println!(
        "Compiled {} shader(s) via miso::stddeferred with defines: {:?}\n",
        shaders.len(),
        defines
    );

    for shader in shaders {
        println!("==============================");
        println!("Stage: {:?}", shader.stage);
        println!("Language: {:?}", shader.lang);
        println!("Name: {:?}", shader.name);
        println!("Source file: {:?}", shader.file);
        println!("Entry points: {:?}", shader.metadata.entry_points);

        if let Some(size) = shader.metadata.workgroup_size {
            println!("Workgroup size: {:?}", size);
        }

        print_interface("Inputs", &shader.metadata.inputs);
        print_interface("Outputs", &shader.metadata.outputs);
        print_bindings(&shader.variables);

        if let Some(vertex) = &shader.metadata.vertex {
            print_vertex_layout(vertex);
        }

        println!();
    }
}
