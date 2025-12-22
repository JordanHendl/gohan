use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=SLANG_LIB_DIR");

    if let Ok(dir) = env::var("SLANG_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
    }

    println!("cargo:rustc-link-lib=slang");
}
