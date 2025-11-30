use std::{error::Error, fmt};

use dashi::GPUError;

#[derive(Debug)]
pub enum FurikakeError {
    BufferMapFailed { source: GPUError },
    BufferUnmapFailed { source: GPUError },
    MissingReservedBinding { name: String },
    ReservedItemTypeMismatch { name: String },
    ResolverReflection { source: String },
}

impl FurikakeError {
    pub fn buffer_map_failed<E: Into<GPUError>>(err: E) -> Self {
        Self::BufferMapFailed { source: err.into() }
    }

    pub fn buffer_unmap_failed<E: Into<GPUError>>(err: E) -> Self {
        Self::BufferUnmapFailed { source: err.into() }
    }
}

impl fmt::Display for FurikakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FurikakeError::BufferMapFailed { source } => {
                write!(f, "failed to map buffer: {}", source)
            }
            FurikakeError::BufferUnmapFailed { source } => {
                write!(f, "failed to unmap buffer: {}", source)
            }
            FurikakeError::MissingReservedBinding { name } => {
                write!(f, "reserved binding `{}` not found", name)
            }
            FurikakeError::ReservedItemTypeMismatch { name } => {
                write!(f, "reserved binding `{}` had the wrong type", name)
            }
            FurikakeError::ResolverReflection { source } => {
                write!(f, "failed to reflect resolver bindings: {}", source)
            }
        }
    }
}

impl Error for FurikakeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FurikakeError::BufferMapFailed { source }
            | FurikakeError::BufferUnmapFailed { source } => Some(source),
            FurikakeError::ResolverReflection { .. }
            | FurikakeError::MissingReservedBinding { .. }
            | FurikakeError::ReservedItemTypeMismatch { .. } => None,
        }
    }
}

impl From<GPUError> for FurikakeError {
    fn from(value: GPUError) -> Self {
        FurikakeError::ResolverReflection {
            source: value.to_string(),
        }
    }
}

impl From<String> for FurikakeError {
    fn from(value: String) -> Self {
        FurikakeError::ResolverReflection { source: value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_mapping_failures() {
        let map_error = FurikakeError::BufferMapFailed {
            source: GPUError::LibraryError(),
        };
        assert_eq!(
            format!("{}", map_error),
            "failed to map buffer: Library could not be loaded"
        );

        let unmap_error = FurikakeError::BufferUnmapFailed {
            source: GPUError::SlotError(),
        };
        assert_eq!(
            format!("{}", unmap_error),
            "failed to unmap buffer: Slot Error"
        );
    }

    #[test]
    fn displays_missing_binding() {
        let missing = FurikakeError::MissingReservedBinding {
            name: "meshi_camera".to_string(),
        };

        assert_eq!(
            format!("{}", missing),
            "reserved binding `meshi_camera` not found"
        );
    }

    #[test]
    fn displays_mismatched_binding_type() {
        let mismatched = FurikakeError::ReservedItemTypeMismatch {
            name: "meshi_camera".to_string(),
        };

        assert_eq!(
            format!("{}", mismatched),
            "reserved binding `meshi_camera` had the wrong type"
        );
    }
}
