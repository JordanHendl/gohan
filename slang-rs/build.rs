use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_REPO_URL: &str = "https://github.com/shader-slang/slang.git";
const DEFAULT_REVISION: &str = "master";
const DEFAULT_VERSION: &str = "2025.24.2";

fn main() {
    println!("cargo:rerun-if-env-changed=SLANG_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SLANG_REPO_URL");
    println!("cargo:rerun-if-env-changed=SLANG_SOURCE_REV");
    println!("cargo:rerun-if-env-changed=SLANG_VERSION");

    if let Some(dir) = env::var_os("SLANG_LIB_DIR") {
        let dir = PathBuf::from(dir);
        if has_slang_library(&dir) {
            println!("cargo:rustc-link-search=native={}", dir.display());
            println!("cargo:rustc-link-lib=slang");
            return;
        }

        println!(
            "cargo:warning=SLANG_LIB_DIR was set to {} but no slang library was found; attempting to fetch it",
            dir.display()
        );
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let cache_dir = out_dir.join("slang-artifacts");
    let lib_dir = cache_dir.join("lib");

    if !has_slang_library(&lib_dir) {
        fs::create_dir_all(&lib_dir).expect("failed to create slang artifact directory");
        let version = env::var("SLANG_VERSION").unwrap_or_else(|_| DEFAULT_VERSION.to_string());
        let target = env::var("TARGET").expect("TARGET not set");

        if !download_prebuilt(&cache_dir, &lib_dir, &version, &target) {
            let source_rev =
                env::var("SLANG_SOURCE_REV").unwrap_or_else(|_| DEFAULT_REVISION.to_string());
            let repo_url =
                env::var("SLANG_REPO_URL").unwrap_or_else(|_| DEFAULT_REPO_URL.to_string());
            build_slang_from_source(&cache_dir, &repo_url, &source_rev);
        }
    }

    if !has_slang_library(&lib_dir) {
        panic!(
            "slang library could not be located in {} even after attempting to fetch it",
            lib_dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=slang");
}

fn has_slang_library(dir: &Path) -> bool {
    dir.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| is_slang_library_name(entry.file_name()))
}

fn download_prebuilt(cache_dir: &Path, lib_dir: &Path, version: &str, target: &str) -> bool {
    let (archive_name, lib_name) = match target {
        t if t.contains("linux") && t.contains("x86_64") => {
            (format!("slang-{version}-linux-x86_64.zip"), "libslang.so")
        }
        t if t.contains("apple-darwin") && t.contains("aarch64") => {
            (format!("slang-{version}-macos-arm64.zip"), "libslang.dylib")
        }
        t if t.contains("apple-darwin") => (
            format!("slang-{version}-macos-x86_64.zip"),
            "libslang.dylib",
        ),
        t if t.contains("windows") => (format!("slang-{version}-windows-x86_64.zip"), "slang.dll"),
        _ => return false,
    };

    let url = format!(
        "https://github.com/shader-slang/slang/releases/download/v{version}/{archive_name}"
    );
    let downloads = cache_dir.join("downloads");
    let archive_path = downloads.join(&archive_name);

    if !archive_path.exists() {
        fs::create_dir_all(&downloads).expect("failed to create download directory");
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                let bytes = response
                    .bytes()
                    .unwrap_or_else(|err| panic!("failed reading archive: {err}"));
                let mut file = fs::File::create(&archive_path)
                    .unwrap_or_else(|err| panic!("failed to create archive file: {err}"));
                file.write_all(&bytes)
                    .unwrap_or_else(|err| panic!("failed to write archive: {err}"));
            }
            Err(error) => {
                println!(
                    "cargo:warning=failed to download prebuilt slang ({error}); building from source instead"
                );
                return false;
            }
        }
    }

    let extract_root = cache_dir.join("prebuilt");
    if !extract_root.exists() {
        fs::create_dir_all(&extract_root).expect("failed to create extraction directory");
    }

    if let Err(error) = unpack_archive(&archive_path, &extract_root) {
        println!(
            "cargo:warning=failed to extract prebuilt slang ({error}); building from source instead"
        );
        return false;
    }

    if let Some(lib_path) = find_prebuilt_library(&extract_root) {
        let dest = lib_dir.join(lib_name);
        copy_library(&lib_path, &dest, "prebuilt");
        return true;
    }

    println!(
        "cargo:warning=prebuilt slang archive did not contain a slang library, falling back to source build"
    );
    false
}

fn find_prebuilt_library(root: &Path) -> Option<PathBuf> {
    find_library_recursive(root)
}

fn unpack_archive(archive_path: &Path, destination: &Path) -> zip::result::ZipResult<()> {
    let file = fs::File::open(archive_path).expect("failed to open downloaded archive");
    let mut archive = zip::ZipArchive::new(file)?;
    archive.extract(destination)
}

fn build_slang_from_source(cache_dir: &Path, repo_url: &str, rev: &str) {
    let source_dir = cache_dir.join("slang-src");
    if !source_dir.exists() {
        run(Command::new("git").args([
            "clone",
            "--recurse-submodules",
            "--shallow-submodules",
            "--depth",
            "1",
            "--branch",
            rev,
            repo_url,
            source_dir.to_str().expect("non-utf8 build path"),
        ]));
    }

    run(Command::new("git")
        .current_dir(&source_dir)
        .args(["fetch", "--tags", "--force", "--depth", "1", "origin", rev]));
    run(Command::new("git")
        .current_dir(&source_dir)
        .args(["checkout", rev]));
    run(Command::new("git").current_dir(&source_dir).args([
        "submodule",
        "update",
        "--init",
        "--recursive",
        "--depth",
        "1",
    ]));

    let build_dir = cache_dir.join("build");
    let mut cmake_config = Command::new("cmake");
    cmake_config
        .arg("-S")
        .arg(&source_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-DSLANG_ENABLE_SLANGC=ON")
        .arg("-DSLANG_BUILD_TESTS=OFF")
        .arg("-DSLANG_ENABLE_TESTS=OFF")
        .arg("-DSLANG_ENABLE_GFX=OFF");
    run(&mut cmake_config);

    run(Command::new("cmake")
        .arg("--build")
        .arg(&build_dir)
        .arg("--target")
        .arg("slang")
        .arg("--config")
        .arg("Release"));

    let built_lib = find_built_library(&build_dir).unwrap_or_else(|| {
        panic!(
            "Unable to locate built slang library under {}",
            build_dir.display()
        )
    });
    let dest = cache_dir.join("lib");
    fs::create_dir_all(&dest).expect("failed to create lib staging directory");
    let filename = built_lib.file_name().expect("missing filename");
    copy_library(&built_lib, &dest.join(filename), "built");
}

fn find_built_library(build_dir: &Path) -> Option<PathBuf> {
    find_library_recursive(build_dir)
}

fn find_library_recursive(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_slang_library_name(entry.file_name()) {
                return Some(path);
            }
        }
    }
    None
}

fn is_slang_library_name(name: OsString) -> bool {
    if let Some(name) = name.to_str() {
        name == "slang.dll"
            || name.starts_with("libslang.so")
            || name.starts_with("libslang.dylib")
            || name == "libslang.a"
    } else {
        false
    }
}

fn copy_library(source: &Path, destination: &Path, source_kind: &str) {
    let metadata = fs::symlink_metadata(source)
        .unwrap_or_else(|err| panic!("failed to inspect {source_kind} library: {err}"));

    let copy_source = if metadata.file_type().is_symlink() {
        let link_target = fs::read_link(source).unwrap_or_else(|err| {
            panic!("failed to read {source_kind} library symlink {source:?}: {err}")
        });
        let resolved = if link_target.is_absolute() {
            link_target
        } else {
            source
                .parent()
                .expect("symlink without parent")
                .join(link_target)
        };
        fs::canonicalize(resolved).unwrap_or_else(|err| {
            panic!(
                "failed to resolve {source_kind} library target from {source:?}: {err}"
            )
        })
    } else {
        source.to_path_buf()
    };

    fs::copy(&copy_source, destination).unwrap_or_else(|err| {
        panic!(
            "failed to copy {source_kind} library from {copy_source:?} to {destination:?}: {err}"
        )
    });
}

fn run(command: &mut Command) {
    let status = command.status().unwrap_or_else(|err| {
        panic!("failed to run {:?}: {}", command, err);
    });
    if !status.success() {
        panic!("command {:?} failed with status {status}", command);
    }
}
