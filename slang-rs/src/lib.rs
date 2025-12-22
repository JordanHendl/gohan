use shader_slang::{self as slang, Downcast};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use tempfile::TempDir;

pub use slang::OptimizationLevel;
pub use slang::Stage;

#[derive(Debug, thiserror::Error)]
pub enum SlangError {
    #[error("failed to create Slang session")]
    SessionUnavailable,
    #[error("failed to find Slang entry point")]
    MissingEntryPoint,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid string data: {0}")]
    Nul(#[from] std::ffi::NulError),
    #[error("slang compilation failed: {0}")]
    Slang(#[from] slang::Error),
    #[error("Slang produced non-SPIR-V output")]
    InvalidSpirv,
}

pub fn compile_to_spirv(
    source: &str,
    stage: Stage,
    optimization: OptimizationLevel,
    debug_symbols: bool,
    defines: &HashMap<String, Option<String>>,
) -> Result<Vec<u32>, SlangError> {
    let temp_dir = TempDir::new()?;
    let module_name = write_module(temp_dir.path(), source)?;

    let search_path = CString::new(temp_dir.path().to_string_lossy().as_bytes())?;
    let search_paths = [search_path.as_ptr()];

    let global_session = slang::GlobalSession::new().ok_or(SlangError::SessionUnavailable)?;
    let target_desc = slang::TargetDesc::default()
        .format(slang::CompileTarget::Spirv)
        .profile(global_session.find_profile("glsl_450"));
    let targets = [target_desc];

    let mut compiler_options = slang::CompilerOptions::default()
        .language(slang::SourceLanguage::Slang)
        .target(slang::CompileTarget::Spirv)
        .stage(stage)
        .optimization(optimization);

    if debug_symbols {
        compiler_options = compiler_options.debug_information(slang::DebugInfoLevel::Standard);
    }

    for (name, value) in defines {
        let value = value.as_deref().unwrap_or("1");
        compiler_options = compiler_options.macro_define(name, value);
    }

    let session_desc = slang::SessionDesc::default()
        .targets(&targets)
        .search_paths(&search_paths)
        .options(&compiler_options);

    let session = global_session
        .create_session(&session_desc)
        .ok_or(SlangError::SessionUnavailable)?;
    let module = session.load_module(&module_name)?;
    let entry_point = module
        .find_entry_point_by_name("main")
        .ok_or(SlangError::MissingEntryPoint)?;

    let composite = session.create_composite_component_type(&[
        module.downcast().clone(),
        entry_point.downcast().clone(),
    ])?;

    let linked = composite.link()?;
    let blob = linked.entry_point_code(0, 0)?;
    let bytes = blob.as_slice();

    if bytes.len() % 4 != 0 {
        return Err(SlangError::InvalidSpirv);
    }

    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();

    Ok(words)
}

fn write_module(dir: &std::path::Path, source: &str) -> Result<String, std::io::Error> {
    let path = dir.join("module.slang");
    fs::write(&path, source)?;
    Ok(path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "module.slang".to_string()))
}
