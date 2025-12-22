extern crate bindgen;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const SLANG_VERSION: &str = "2025.24.2";

fn main() {
    println!("cargo:rerun-if-env-changed=SLANG_DIR");
    println!("cargo:rerun-if-env-changed=SLANG_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=SLANG_LIB_DIR");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    let (include_dir, lib_dir) = locate_or_download_slang().expect("Unable to locate Slang SDK");

    if !lib_dir.as_os_str().is_empty() {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
    }
    println!("cargo:rustc-link-lib=dylib=slang");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Couldn't determine output directory."));
    let header_path = include_dir.join("slang.h");

    let bindings = bindgen::builder()
        .header(header_path.to_string_lossy())
        .clang_arg("-v")
        .clang_arg("-xc++")
        .clang_arg("-std=c++17")
        .clang_arg(format!("-I{}", include_dir.display()))
        .allowlist_function("spReflection.*")
        .allowlist_function("spComputeStringHash")
        .allowlist_function("slang_.*")
        .allowlist_type("slang.*")
        .allowlist_var("SLANG_.*")
        .with_codegen_config(
            bindgen::CodegenConfig::FUNCTIONS
                | bindgen::CodegenConfig::TYPES
                | bindgen::CodegenConfig::VARS,
        )
        .parse_callbacks(Box::new(ParseCallback {}))
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .constified_enum("SlangProfileID")
        .constified_enum("SlangCapabilityID")
        .vtable_generation(true)
        .layout_tests(false)
        .derive_copy(true)
        .generate()
        .expect("Couldn't generate bindings.");

    let mut output = bindings.to_string();
    output = output.replace("extern \"C\" {", "unsafe extern \"C\" {");

    fs::write(out_dir.join("bindings.rs"), output).expect("Couldn't write bindings.");
}

fn locate_or_download_slang() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    if let Ok(dir) = env::var("SLANG_INCLUDE_DIR") {
        let include_dir = PathBuf::from(dir);
        let lib_dir = env::var("SLANG_LIB_DIR").map(PathBuf::from).unwrap_or_default();
        return Ok((include_dir, lib_dir));
    }

    if let Ok(dir) = env::var("SLANG_DIR") {
        return Ok((PathBuf::from(&dir).join("include"), PathBuf::from(dir).join("lib")));
    }

    if let Ok(dir) = env::var("VULKAN_SDK") {
        return Ok((PathBuf::from(&dir).join("include/slang"), PathBuf::from(dir).join("lib")));
    }

    println!("cargo:warning=Downloading Slang SDK v{} for local build", SLANG_VERSION);
    download_slang_release()
}

fn download_slang_release() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let install_root = out_dir.join("slang-sdk");

    let include_hint = install_root.join("include").join("slang.h");
    if include_hint.exists() {
        let root = find_install_root(&install_root)?;
        return Ok((root.join("include"), root.join("lib")));
    }

    fs::create_dir_all(&install_root)?;

    let url = format!(
        "https://github.com/shader-slang/slang/releases/download/v{0}/slang-{0}-linux-x86_64.zip",
        SLANG_VERSION
    );

    let bytes = fetch_archive(&url)?;

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("failed to unpack archive: {e}")))?;
    archive
        .extract(&install_root)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("failed to extract archive: {e}")))?;

    let root = find_install_root(&install_root)?;
    let lib_dir = root.join("lib");
    normalize_library_links(&lib_dir)?;

    Ok((root.join("include"), lib_dir))
}

fn fetch_archive(url: &str) -> io::Result<Vec<u8>> {
    fetch_with_agent(url).or_else(|primary_err| fetch_with_curl(url, primary_err))
}

fn fetch_with_agent(url: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let response = download_agent()
        .get(url)
        .call()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("failed to read download: {e}")))?;

    Ok(bytes)
}

fn fetch_with_curl(url: &str, previous_error: io::Error) -> io::Result<Vec<u8>> {
    let output = std::process::Command::new("curl")
        .arg("-kL")
        .arg(url)
        .output()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("curl failed: {e}")))?;

    if output.status.success() {
        return Ok(output.stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("{previous_error}; curl fallback: {stderr}"),
    ))
}

fn download_agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();

    if let Some(proxy_url) = proxy_url() {
        if let Ok(proxy) = ureq::Proxy::new(proxy_url) {
            builder = builder.proxy(proxy);
        }
    }

    builder.build()
}

fn proxy_url() -> Option<String> {
    [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ]
    .into_iter()
    .find_map(|key| env::var(key).ok())
}

fn find_install_root(base: &Path) -> io::Result<PathBuf> {
    if base.join("include/slang.h").exists() && base.join("lib").is_dir() {
        return Ok(base.to_path_buf());
    }

    for entry in fs::read_dir(base)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }

        let include_path = path.join("include/slang.h");
        if include_path.exists() && path.join("lib").is_dir() {
            return Ok(path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Unable to locate extracted Slang SDK",
    ))
}

fn normalize_library_links(lib_dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(lib_dir)? {
        let path = entry?.path();

        if !path.is_file() {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        if metadata.len() > 256 {
            continue;
        }

        let Ok(target_name) = fs::read_to_string(&path) else {
            continue;
        };

        let target_path = lib_dir.join(target_name.trim());
        if target_path.exists() {
            fs::copy(&target_path, &path)?;
        }
    }

    Ok(())
}

#[derive(Debug)]
struct ParseCallback {}

impl bindgen::callbacks::ParseCallbacks for ParseCallback {
    fn enum_variant_name(
        &self,
        enum_name: Option<&str>,
        original_variant_name: &str,
        _variant_value: bindgen::callbacks::EnumVariantValue,
    ) -> Option<String> {
        let enum_name = enum_name?;

        let mut map = std::collections::HashMap::new();
        map.insert("SlangMatrixLayoutMode", "SlangMatrixLayout");
        map.insert("SlangCompileTarget", "Slang");

        let trim = map.get(enum_name).unwrap_or(&enum_name);
        let new_variant_name = pascal_case_from_snake_case(original_variant_name);
        let new_variant_name = new_variant_name.trim_start_matches(trim);
        Some(new_variant_name.to_string())
    }
}

fn pascal_case_from_snake_case(snake_case: &str) -> String {
    let mut result = String::new();

    let should_lower = snake_case
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase());

    for part in snake_case.split('_') {
        for (i, c) in part.chars().enumerate() {
            if i == 0 {
                result.push(c.to_ascii_uppercase());
            } else if should_lower {
                result.push(c.to_ascii_lowercase());
            } else {
                result.push(c);
            }
        }
    }

    result
}
