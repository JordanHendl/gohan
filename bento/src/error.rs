use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingBinding {
    pub name: String,
    pub set: u32,
    pub binding: u32,
}

#[derive(Debug, Error)]
pub enum PipelineBuildError {
    #[error("Missing {stage} shader stage")]
    MissingShader { stage: &'static str },

    #[error("Missing shader bindings: {bindings:?}")]
    MissingBindings { bindings: Vec<MissingBinding> },

    #[error(
        "Mismatched binding counts for set {set} binding {binding}: expected {expected}, provided {provided}"
    )]
    MismatchedBindingCounts {
        set: u32,
        binding: u32,
        expected: u32,
        provided: u32,
    },

    #[error("Invalid resource count for {name}: expected {expected}, provided {provided}")]
    InvalidResourceCount {
        name: String,
        expected: u32,
        provided: u32,
    },

    #[error("Invalid resource slots for {name}: expected slots 0..{expected}")]
    InvalidResourceSlots { name: String, expected: u32 },

    #[error("Failed to create default {resource_type} resource for {name}: {source}")]
    DefaultResourceCreateFailed {
        name: String,
        resource_type: &'static str,
        #[source]
        source: dashi::GPUError,
    },

    #[error("Failed to create bind table layout for set {set}: {source}")]
    BindTableLayoutCreateFailed {
        set: u32,
        #[source]
        source: dashi::GPUError,
    },

    #[error("Failed to create bind table for set {set}: {source}")]
    BindTableCreateFailed {
        set: u32,
        #[source]
        source: dashi::GPUError,
    },

    #[error("Failed to create {pipeline} pipeline layout: {source}")]
    PipelineLayoutCreateFailed {
        pipeline: &'static str,
        #[source]
        source: dashi::GPUError,
    },

    #[error("Failed to create {pipeline} pipeline: {source}")]
    PipelineCreateFailed {
        pipeline: &'static str,
        #[source]
        source: dashi::GPUError,
    },
}

/// Error variants surfaced by Bento shader compilation and inspection routines.
#[derive(Debug, Error)]
pub enum BentoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Binary serialization error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("Shader compilation error: {0}")]
    ShaderCompilation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Pipeline build error: {0}")]
    PipelineBuild(#[from] PipelineBuildError),

    #[error("Shader backend error: {0}")]
    Dashi(#[from] dashi::GPUError),
}
