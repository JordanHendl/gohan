use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Recursively traverses a directory and collects all *.slang files.
fn collect_slang_files(dir: &Path, slang_files: &mut Vec<PathBuf>) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("Failed to read directory") {
            let entry = entry.expect("Failed to get entry");
            let path = entry.path();
            if path.is_dir() {
                collect_slang_files(&path, slang_files);
            } else if let Some(ext) = path.extension() {
                if ext == "slang" {
                    slang_files.push(path);
                }
            }
        }
    }
}

fn main() {
    // Get the source and output directories from environment variables.
    let out_dir = env::var("SLANG_OUT_DIR").unwrap_or_else(|_| "target/spirv".to_string());
    let src_dir = env::var("SLANG_SRC_DIR").unwrap_or_else(|_| "src/slang/src".to_string());
    let include_dir = env::var("SLANG_INCLUDE_DIR").unwrap_or_else(|_| "src/slang/include/".to_string());

    let src_path = Path::new(&src_dir);
    let out_path = Path::new(&out_dir);

    // Ensure the output directory exists.
    if !out_path.exists() {
        fs::create_dir_all(out_path).expect("Failed to create output directory");
    }

    // Collect all .slang files from the source directory.
    let mut slang_files = Vec::new();
    collect_slang_files(src_path, &mut slang_files);

    // Compile each .slang file.
    for input_file in &slang_files {
        let relative_path = input_file.strip_prefix(src_path).expect("Failed to strip prefix");
        let mut output_file = out_path.join(relative_path);
        output_file.set_extension("spv");

        // Ensure the output subdirectory exists.
        if let Some(parent) = output_file.parent() {
            fs::create_dir_all(parent).expect("Failed to create output subdirectories");
        }

        println!(
            "cargo:rerun-if-changed={}",
            input_file.to_str().expect("Invalid input file path")
        );

//        // Run the `slang` command.
//        let status = Command::new("slangc")
//            .arg(input_file)
//            .arg("-profile")
//            .arg("glsl_460")
//            .arg("-target")
//            .arg("spirv")
//            .arg("-capability")
//            .arg("glsl_spirv")
//            .arg("-I")
//            .arg(&include_dir)
//            .arg("-fspv-reflect")
//            .arg("-force-glsl-scalar-layout")
//            .arg("-fvk-use-gl-layout")
//            .arg("-fvk-invert-y")
////            .arg("-emit-spirv-via-glsl")
//            .arg("-Xdxc")
//            .arg("--reflect-all-block-variables")
//            .arg("-g1")
//            .arg("-entry")
//            .arg("main")
//            .arg("-o")
//            .arg(&output_file)
//            .status()
//            .expect("Failed to execute slang command");
//
//        if !status.success() {
//            panic!(
//                "Slang compilation failed for file: {}",
//                input_file.display()
//            );
//        }
//
//        println!(
//            "Compiled {} to {}",
//            input_file.display(),
//            output_file.display()
//        );
    }
}

