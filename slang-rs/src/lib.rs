#![deny(missing_docs)]

//! Low level Rust bindings to the shaded [Slang](https://github.com/shader-slang/slang)
//! language compiler library.
//!
//! The bindings surface a handful of the C API entry points that are useful for
//! creating global sessions and loading modules. Linking against the native
//! `slang` library is expected; provide `SLANG_LIB_DIR` at build time to point at
//! a directory containing the compiled shared object or static library.

use std::ffi::{c_char, c_void};
use std::marker::PhantomData;

/// Signed integer type used by the Slang API.
pub type SlangInt = isize;

/// Result type returned by many Slang functions.
pub type SlangResult = i32;

/// Successful result code.
pub const SLANG_OK: SlangResult = 0;
/// Current Slang API version supported by the bindings.
pub const SLANG_API_VERSION: u32 = 0;
/// Legacy language version constant used by Slang.
pub const SLANG_LANGUAGE_VERSION_LEGACY: u32 = 2018;
/// Slang 2025 language version constant.
pub const SLANG_LANGUAGE_VERSION_2025: u32 = 2025;
/// Slang 2026 language version constant.
pub const SLANG_LANGUAGE_VERSION_2026: u32 = 2026;
/// Latest available language version constant.
pub const SLANG_LANGUAGE_VERSION_LATEST: u32 = SLANG_LANGUAGE_VERSION_2026;

/// Description of a Slang global session used when constructing the compiler API.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SlangGlobalSessionDesc {
    /// Size of the struct, mirroring `structureSize` in `slang.h`.
    pub structure_size: u32,
    /// Slang API version to target.
    pub api_version: u32,
    /// Minimum language version accepted by sessions created with this descriptor.
    pub min_language_version: u32,
    /// Whether GLSL support should be enabled.
    pub enable_glsl: bool,
    /// Reserved future-proofing slots.
    pub reserved: [u32; 16],
}

impl Default for SlangGlobalSessionDesc {
    fn default() -> Self {
        Self {
            structure_size: std::mem::size_of::<Self>() as u32,
            api_version: SLANG_API_VERSION,
            min_language_version: SLANG_LANGUAGE_VERSION_2025,
            enable_glsl: false,
            reserved: [0; 16],
        }
    }
}

/// Opaque handle to the global compiler session.
#[repr(C)]
pub struct IGlobalSession {
    _private: [u8; 0],
    _marker: PhantomData<()>,
}

/// Opaque handle to a session.
#[repr(C)]
pub struct ISession {
    _private: [u8; 0],
    _marker: PhantomData<()>,
}

/// Opaque handle to a compiled module.
#[repr(C)]
pub struct IModule {
    _private: [u8; 0],
    _marker: PhantomData<()>,
}

/// Opaque buffer interface used throughout the API.
#[repr(C)]
pub struct ISlangBlob {
    _private: [u8; 0],
    _marker: PhantomData<()>,
}

#[link(name = "slang")]
unsafe extern "C" {
    /// Returns the build tag string for the linked Slang library.
    pub fn spGetBuildTagString() -> *const c_char;

    /// Creates a new global Slang session using the built-in core module.
    pub fn slang_createGlobalSession(
        api_version: SlangInt,
        out_global_session: *mut *mut IGlobalSession,
    ) -> SlangResult;

    /// Creates a new global session using the provided descriptor.
    pub fn slang_createGlobalSession2(
        desc: *const SlangGlobalSessionDesc,
        out_global_session: *mut *mut IGlobalSession,
    ) -> SlangResult;

    /// Creates a global session without setting up the core module.
    pub fn slang_createGlobalSessionWithoutCoreModule(
        api_version: SlangInt,
        out_global_session: *mut *mut IGlobalSession,
    ) -> SlangResult;

    /// Returns an embedded core module blob when present.
    pub fn slang_getEmbeddedCoreModule() -> *mut ISlangBlob;

    /// Releases global allocations used by Slang.
    pub fn slang_shutdown();

    /// Returns the last internal error message signaled by the runtime.
    pub fn slang_getLastInternalErrorMessage() -> *const c_char;

    /// Creates a blob from raw memory.
    pub fn slang_createBlob(data: *const c_void, size: usize) -> *mut ISlangBlob;

    /// Loads a module from a source string.
    pub fn slang_loadModuleFromSource(
        session: *mut ISession,
        module_name: *const c_char,
        path: *const c_char,
        source: *const c_char,
        source_size: usize,
        out_diagnostics: *mut *mut ISlangBlob,
    ) -> *mut IModule;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_descriptor_matches_header_defaults() {
        let desc = SlangGlobalSessionDesc::default();
        assert_eq!(
            desc.structure_size as usize,
            std::mem::size_of::<SlangGlobalSessionDesc>()
        );
        assert_eq!(desc.api_version, SLANG_API_VERSION);
        assert_eq!(desc.min_language_version, SLANG_LANGUAGE_VERSION_2025);
        assert!(!desc.enable_glsl);
        assert!(desc.reserved.iter().all(|&value| value == 0));
    }
}
