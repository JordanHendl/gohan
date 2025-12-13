pub mod error;
pub mod builder;
use std::{collections::HashMap, fs, path::Path};

use regex::Regex;
use serde::{Deserialize, Serialize};
use shaderc::{
    CompileOptions, Compiler as ShadercCompiler, EnvVersion, OptimizationLevel as ShadercOpt,
    ShaderKind, SourceLanguage, SpirvVersion, TargetEnv,
};
use rspirv::{
    binary::Assemble,
    dr::{Instruction, Operand},
    spirv,
};

pub use error::*;

/// Supported input languages for Bento shader compilation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShaderLang {
    Infer,
    Slang,
    Glsl,
    Hlsl,
    Other,
}

/// Controls how aggressively Bento optimizes shader bytecode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OptimizationLevel {
    None,
    FileSize,
    Performance,
}

/// Representation of a bind group variable discovered during reflection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShaderVariable {
    pub name: String,
    #[serde(default)]
    pub set: u32,
    pub kind: dashi::BindGroupVariable,
}

/// Stage-specific metadata discovered during reflection.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShaderMetadata {
    pub entry_points: Vec<String>,
    pub inputs: Vec<InterfaceVariable>,
    pub outputs: Vec<InterfaceVariable>,
    pub workgroup_size: Option<[u32; 3]>,
    #[serde(default)]
    pub vertex: Option<VertexLayout>,
}

/// Representation of a shader interface variable (inputs/outputs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterfaceVariable {
    pub name: String,
    pub location: Option<u32>,
    #[serde(default)]
    pub format: Option<dashi::ShaderPrimitiveType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VertexEntry {
    pub format: dashi::ShaderPrimitiveType,
    pub location: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexLayout {
    pub entries: Vec<VertexEntry>,
    pub stride: usize,
    pub rate: dashi::VertexRate,
}

impl PartialEq for VertexLayout {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.stride == other.stride
            && std::mem::discriminant(&self.rate) == std::mem::discriminant(&other.rate)
    }
}

impl Eq for VertexLayout {}

/// Parameters describing how a shader should be compiled into a Bento File.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    pub name: Option<String>,
    pub lang: ShaderLang,
    pub stage: dashi::ShaderType,
    pub optimization: OptimizationLevel,
    pub debug_symbols: bool,
}

/// Serialized result produced after compiling a shader into the Bento Format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompilationResult {
    pub name: Option<String>,
    pub file: Option<String>,
    pub lang: ShaderLang,
    pub stage: dashi::ShaderType,
    pub variables: Vec<ShaderVariable>,
    pub metadata: ShaderMetadata,
    pub spirv: Vec<u32>,
}

/// Identifies whether a pipeline is used for graphics rendering or compute workloads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PipelineKind {
    Graphics,
    Compute,
}

/// Convenience container that groups together compatible shader stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Pipeline {
    Graphics(GraphicsPipeline),
    Compute(ComputePipeline),
}

/// A graphics pipeline made from vertex and fragment shader results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphicsPipeline {
    pub vertex: CompilationResult,
    pub fragment: CompilationResult,
}

/// A compute pipeline that contains a single compute shader result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputePipeline {
    pub compute: CompilationResult,
}

impl Pipeline {
    /// Creates a pipeline from an arbitrary collection of stage compilation results.
    ///
    /// * Graphics pipelines require both a vertex stage and a fragment stage.
    /// * Compute pipelines contain exactly one compute stage and cannot be mixed with graphics stages.
    pub fn from_stages<I>(stages: I) -> Result<Self, BentoError>
    where
        I: IntoIterator<Item = CompilationResult>,
    {
        let mut vertex: Option<CompilationResult> = None;
        let mut fragment: Option<CompilationResult> = None;
        let mut compute: Option<CompilationResult> = None;

        for stage in stages {
            match stage.stage {
                dashi::ShaderType::Vertex => {
                    if vertex.replace(stage).is_some() {
                        return Err(BentoError::InvalidInput(
                            "Graphics pipelines can only contain one vertex stage".into(),
                        ));
                    }
                }
                dashi::ShaderType::Fragment => {
                    if fragment.replace(stage).is_some() {
                        return Err(BentoError::InvalidInput(
                            "Graphics pipelines can only contain one fragment stage".into(),
                        ));
                    }
                }
                dashi::ShaderType::Compute => {
                    if compute.replace(stage).is_some() {
                        return Err(BentoError::InvalidInput(
                            "Compute pipelines can only contain one compute stage".into(),
                        ));
                    }
                }
                dashi::ShaderType::All => {
                    return Err(BentoError::InvalidInput(
                        "ShaderType::All cannot be used to build a pipeline".into(),
                    ));
                }
            }
        }

        if let Some(compute) = compute {
            if vertex.is_some() || fragment.is_some() {
                return Err(BentoError::InvalidInput(
                    "Compute pipelines cannot include graphics stages".into(),
                ));
            }

            return Ok(Self::Compute(ComputePipeline { compute }));
        }

        let vertex = vertex.ok_or_else(|| {
            BentoError::InvalidInput("Graphics pipelines require a vertex stage".into())
        })?;

        let fragment = fragment.ok_or_else(|| {
            BentoError::InvalidInput("Graphics pipelines require a fragment stage".into())
        })?;

        Ok(Self::Graphics(GraphicsPipeline { vertex, fragment }))
    }

    /// Returns the type of pipeline represented by this instance.
    pub fn kind(&self) -> PipelineKind {
        match self {
            Self::Graphics(_) => PipelineKind::Graphics,
            Self::Compute(_) => PipelineKind::Compute,
        }
    }

    /// Returns the vertex shader stage, if the pipeline is graphics.
    pub fn vertex(&self) -> Option<&CompilationResult> {
        match self {
            Self::Graphics(graphics) => Some(&graphics.vertex),
            Self::Compute(_) => None,
        }
    }

    /// Returns the fragment shader stage, if available.
    pub fn fragment(&self) -> Option<&CompilationResult> {
        match self {
            Self::Graphics(graphics) => Some(&graphics.fragment),
            Self::Compute(_) => None,
        }
    }

    /// Returns the compute shader stage, if the pipeline is compute.
    pub fn compute(&self) -> Option<&CompilationResult> {
        match self {
            Self::Graphics(_) => None,
            Self::Compute(compute) => Some(&compute.compute),
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
impl ShaderMetadata {
    pub fn vertex_inputs(&self) -> Vec<dashi::VertexEntryInfo> {
        self.inputs
            .iter()
            .map(|a| {
                // TODO: Get format from input, location, and calculate offsets assuming data is
                // packed.
                dashi::VertexEntryInfo {
                    format: todo!(),
                    location: todo!(),
                    offset: todo!(),
                }
            })
            .collect()
    }
}

impl CompilationResult {
    pub fn save_to_disk(&self, path: &str) -> Result<(), BentoError> {
        let path = Path::new(path);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let bytes = self.to_bytes()?;
        fs::write(path, bytes)?;

        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, BentoError> {
        Ok(bincode::serialize(self)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BentoError> {
        Ok(bincode::deserialize(bytes)?)
    }

    pub fn load_from_disk(path: &str) -> Result<Self, BentoError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    pub fn bind_group_variables(&self) -> Vec<dashi::BindGroupVariable> {
        let s: Vec<dashi::BindGroupVariable> =
            self.variables.iter().map(|a| a.kind.clone()).collect();

        return s;
    }
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

/// High-level wrapper around shaderc that emits Bento Files.
pub struct Compiler {
    compiler: ShadercCompiler,
}

impl Compiler {
    pub fn new() -> Result<Self, BentoError> {
        let compiler = ShadercCompiler::new()
            .ok_or_else(|| BentoError::ShaderCompilation("Failed to initialize compiler".into()))?;

        Ok(Self { compiler })
    }

    pub fn compile(
        &self,
        shader: &[u8],
        request: &Request,
    ) -> Result<CompilationResult, BentoError> {
        self.compile_with_path(shader, request, None)
    }

    pub fn compile_from_file(
        &self,
        path: &str,
        request: &Request,
    ) -> Result<CompilationResult, BentoError> {
        let bytes = fs::read(path)
            .map_err(|e| BentoError::Io(std::io::Error::new(e.kind(), format!("{path}: {e}"))))?;
        let mut result = self.compile_with_path(&bytes, request, Some(path))?;
        result.file = Some(path.to_string());

        Ok(result)
    }

    fn compile_with_path(
        &self,
        shader: &[u8],
        request: &Request,
        path: Option<&str>,
    ) -> Result<CompilationResult, BentoError> {
        let source = std::str::from_utf8(shader)
            .map_err(|_| BentoError::InvalidInput("Shader source is not valid UTF-8".into()))?;

        let mut options = CompileOptions::new()
            .ok_or_else(|| BentoError::ShaderCompilation("Failed to create options".into()))?;

        let resolved_lang = if matches!(request.lang, ShaderLang::Infer) {
            infer_shader_lang(source, path)
        } else {
            request.lang
        };

        options.set_source_language(source_language(resolved_lang)?);
        options.set_target_env(TargetEnv::Vulkan, EnvVersion::Vulkan1_2 as u32);
        options.set_target_spirv(SpirvVersion::V1_3);
        options.set_optimization_level(shaderc_optimization(request.optimization));

        if request.debug_symbols {
            options.set_generate_debug_info();
        }

        let shader_kind = shader_stage(request.stage)?;

        let artifact = self
            .compiler
            .compile_into_spirv(
                source,
                shader_kind,
                request.name.as_deref().unwrap_or("shader"),
                "main",
                Some(&options),
            )
            .map_err(|e| BentoError::ShaderCompilation(e.to_string()))?;

        let mut spirv = artifact.as_binary().to_vec();
        let spirv_bytes = spirv_words_to_bytes(&spirv);
        let variables = reflect_bindings(spirv_bytes, source, resolved_lang)?;
        spirv = rewrite_spirv_binding_names(&spirv, &variables)?;
        let spirv_bytes = spirv_words_to_bytes(&spirv);
        let metadata = reflect_metadata(spirv_bytes)?;

        Ok(CompilationResult {
            name: request.name.clone(),
            file: None,
            lang: resolved_lang,
            stage: request.stage,
            variables,
            metadata,
            spirv,
        })
    }
}

fn shader_stage(stage: dashi::ShaderType) -> Result<ShaderKind, BentoError> {
    match stage {
        dashi::ShaderType::Vertex => Ok(ShaderKind::Vertex),
        dashi::ShaderType::Fragment => Ok(ShaderKind::Fragment),
        dashi::ShaderType::Compute => Ok(ShaderKind::Compute),
        dashi::ShaderType::All => Err(BentoError::InvalidInput(
            "ShaderType::All is not supported for compilation".into(),
        )),
    }
}

fn infer_shader_lang(source: &str, filename: Option<&str>) -> ShaderLang {
    let mut detected: Option<ShaderLang> = None;

    if let Some(extension) = filename
        .and_then(|path| Path::new(path).extension())
        .and_then(|ext| ext.to_str())
    {
        match extension.to_ascii_lowercase().as_str() {
            "slang" => detected = Some(ShaderLang::Slang),
            "hlsl" | "hlsli" => detected = Some(ShaderLang::Hlsl),
            "glsl" | "vert" | "frag" | "comp" => detected = Some(ShaderLang::Glsl),
            _ => {}
        }
    }

    for line in source.lines().map(str::trim) {
        if detected.is_some() {
            break;
        }

        if line.is_empty() {
            continue;
        }

        if line.starts_with("import ") || line.starts_with("module ") {
            detected = Some(ShaderLang::Slang);
            break;
        }

        if line.starts_with("#version") || line.starts_with("#extension") {
            detected = Some(ShaderLang::Glsl);
            break;
        }

        if line.contains(": SV_")
            || line.contains(": register(")
            || line.contains("[[vk::binding")
            || line.contains("cbuffer")
            || line.contains("RWStructuredBuffer")
            || line.contains("StructuredBuffer")
            || line.contains("SamplerState")
            || line.contains("Texture2D")
            || line.contains("Texture3D")
            || line.contains("TextureCube")
            || line.contains("[numthreads(")
        {
            detected = Some(ShaderLang::Hlsl);
            break;
        }

        if line.contains("layout(") || line.contains("gl_") {
            detected = Some(ShaderLang::Glsl);
            break;
        }
    }

    detected.unwrap_or(ShaderLang::Glsl)
}

fn source_language(lang: ShaderLang) -> Result<SourceLanguage, BentoError> {
    match lang {
        ShaderLang::Glsl => Ok(SourceLanguage::GLSL),
        ShaderLang::Hlsl | ShaderLang::Slang => Ok(SourceLanguage::HLSL),
        ShaderLang::Other => Err(BentoError::InvalidInput(
            "Unsupported shader language".into(),
        )),
        ShaderLang::Infer => Ok(SourceLanguage::GLSL),
    }
}

fn shaderc_optimization(level: OptimizationLevel) -> ShadercOpt {
    match level {
        OptimizationLevel::None => ShadercOpt::Zero,
        OptimizationLevel::FileSize => ShadercOpt::Size,
        OptimizationLevel::Performance => ShadercOpt::Performance,
    }
}

#[derive(Debug, Clone)]
struct SourceBinding {
    set: u32,
    binding: Option<u32>,
    name: String,
    order: usize,
}

fn spirv_words_to_bytes(words: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(words.as_ptr() as *const u8, words.len() * 4) }
}

fn reflect_bindings(
    spirv_bytes: &[u8],
    source: &str,
    lang: ShaderLang,
) -> Result<Vec<ShaderVariable>, BentoError> {
    use rspirv_reflect::{BindingCount, DescriptorType, Reflection};

    let reflection = Reflection::new_from_spirv(spirv_bytes)
        .map_err(|e| BentoError::ShaderCompilation(e.to_string()))?;

    let mut source_bindings = parse_source_bindings(source, lang)?;
    let descriptor_sets = reflection
        .get_descriptor_sets()
        .map_err(|e| BentoError::ShaderCompilation(e.to_string()))?;

    let mut variables = Vec::new();

    for (set, bindings) in descriptor_sets.iter() {
        for (binding, info) in bindings.iter() {
            let name = take_source_name(*set, *binding, &mut source_bindings)
                .unwrap_or_else(|| info.name.clone());

            let var_type = match info.ty {
                DescriptorType::UNIFORM_BUFFER => dashi::BindGroupVariableType::Uniform,
                DescriptorType::UNIFORM_BUFFER_DYNAMIC => {
                    dashi::BindGroupVariableType::DynamicUniform
                }
                DescriptorType::STORAGE_BUFFER => dashi::BindGroupVariableType::Storage,
                DescriptorType::STORAGE_BUFFER_DYNAMIC => {
                    dashi::BindGroupVariableType::DynamicStorage
                }
                DescriptorType::SAMPLED_IMAGE => dashi::BindGroupVariableType::SampledImage,
                DescriptorType::STORAGE_IMAGE => dashi::BindGroupVariableType::StorageImage,
                DescriptorType::COMBINED_IMAGE_SAMPLER => {
                    dashi::BindGroupVariableType::SampledImage
                }
                _ => dashi::BindGroupVariableType::Uniform,
            };

            let count = match info.binding_count {
                BindingCount::One => 1,
                BindingCount::StaticSized(value) => value as u32,
                BindingCount::Unbounded => 0,
            };

            variables.push(ShaderVariable {
                name,
                set: *set,
                kind: dashi::BindGroupVariable {
                    var_type,
                    binding: *binding,
                    count,
                },
            });
        }
    }

    variables.sort_by(|a, b| {
        a.set
            .cmp(&b.set)
            .then_with(|| a.kind.binding.cmp(&b.kind.binding))
    });

    Ok(variables)
}

fn rewrite_spirv_binding_names(
    spirv: &[u32],
    variables: &[ShaderVariable],
) -> Result<Vec<u32>, BentoError> {
    use rspirv_reflect::Reflection;

    let reflection = Reflection::new_from_spirv(spirv_words_to_bytes(spirv))
        .map_err(|e| BentoError::ShaderCompilation(e.to_string()))?;
    let mut module = reflection.0;

    let mut binding_targets: HashMap<u32, (Option<u32>, Option<u32>)> = HashMap::new();

    for annotation in &module.annotations {
        if annotation.class.opcode != spirv::Op::Decorate {
            continue;
        }

        let Some(Operand::IdRef(id)) = annotation.operands.get(0) else {
            continue;
        };
        let Some(Operand::Decoration(decoration)) = annotation.operands.get(1) else {
            continue;
        };
        let Some(Operand::LiteralBit32(value)) = annotation.operands.get(2) else {
            continue;
        };

        let entry = binding_targets.entry(*id).or_default();

        match decoration {
            spirv::Decoration::DescriptorSet => entry.0 = Some(*value),
            spirv::Decoration::Binding => entry.1 = Some(*value),
            _ => {}
        }
    }

    let mut ids_by_binding = HashMap::new();
    for (id, (set, binding)) in binding_targets {
        if let (Some(set), Some(binding)) = (set, binding) {
            ids_by_binding.insert((set, binding), id);
        }
    }

    for variable in variables {
        let Some(id) = ids_by_binding.get(&(variable.set, variable.kind.binding)) else {
            continue;
        };

        let mut renamed = false;
        for instruction in module.debug_names.iter_mut() {
            if instruction.class.opcode != spirv::Op::Name {
                continue;
            }

            match instruction.operands.get(0) {
                Some(Operand::IdRef(existing)) if existing == id => {
                    if instruction.operands.len() >= 2 {
                        instruction.operands[1] = Operand::LiteralString(variable.name.clone());
                    } else {
                        instruction
                            .operands
                            .push(Operand::LiteralString(variable.name.clone()));
                    }
                    renamed = true;
                    break;
                }
                _ => {}
            }
        }

        if !renamed {
            module.debug_names.push(Instruction::new(
                spirv::Op::Name,
                None,
                None,
                vec![
                    Operand::IdRef(*id),
                    Operand::LiteralString(variable.name.clone()),
                ],
            ));
        }
    }

    Ok(module.assemble())
}

fn parse_source_bindings(source: &str, lang: ShaderLang) -> Result<Vec<SourceBinding>, BentoError> {
    match lang {
        ShaderLang::Glsl => parse_glsl_bindings(source),
        ShaderLang::Hlsl | ShaderLang::Slang => parse_hlsl_like_bindings(source),
        ShaderLang::Other => Err(BentoError::InvalidInput(
            "Unsupported shader language for reflection".into(),
        )),
        ShaderLang::Infer => parse_glsl_bindings(source),
    }
}

fn parse_glsl_bindings(source: &str) -> Result<Vec<SourceBinding>, BentoError> {
    let regex =
        Regex::new(r"layout\s*\(\s*set\s*=\s*(\d+)\s*,\s*binding\s*=\s*(\d+)\s*\)\s*([^;]+);")
            .map_err(|e| {
                BentoError::ShaderCompilation(format!("Invalid GLSL reflection regex: {e}"))
            })?;

    Ok(regex
        .captures_iter(source)
        .enumerate()
        .filter_map(|(index, captures)| {
            let set = captures
                .get(1)
                .and_then(|m| m.as_str().parse::<u32>().ok())?;
            let binding = captures
                .get(2)
                .and_then(|m| m.as_str().parse::<u32>().ok())?;
            let declaration = captures.get(3).map(|m| m.as_str()).unwrap_or("");

            let name = extract_name(declaration)?;

            Some(SourceBinding {
                set,
                binding: Some(binding),
                name,
                order: index,
            })
        })
        .collect())
}

fn parse_hlsl_like_bindings(source: &str) -> Result<Vec<SourceBinding>, BentoError> {
    let resource_regex = Regex::new(
        r"(?m)^\s*(?:RW?Texture\w+|RW?StructuredBuffer|StructuredBuffer|ByteAddressBuffer|RaytracingAccelerationStructure|AccelerationStructure|Texture\w+|Sampler\w*)[^;\n]*?\s+([A-Za-z_][A-Za-z0-9_]*)[^;\n]*?(?::\s*register\(\s*[tubcs]?\s*(\d+)\s*\))?",
    )
    .map_err(|e| BentoError::ShaderCompilation(format!("Invalid HLSL reflection regex: {e}")))?;

    let cbuffer_regex = Regex::new(
        r"(?m)^\s*(?:cbuffer|ConstantBuffer)\s+([A-Za-z_][A-Za-z0-9_]*)[^;\n]*?(?::\s*register\(\s*b(\d+)\s*\))?",
    )
    .map_err(|e| BentoError::ShaderCompilation(format!("Invalid constant buffer regex: {e}")))?;

    let mut bindings = Vec::new();

    for (index, captures) in resource_regex.captures_iter(source).enumerate() {
        let name = captures
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("resource_{index}"));
        let binding = captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok());

        bindings.push(SourceBinding {
            set: 0,
            binding,
            name,
            order: index,
        });
    }

    let starting_index = bindings.len();
    for (offset, captures) in cbuffer_regex.captures_iter(source).enumerate() {
        let name = captures
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("cbuffer_{offset}"));
        let binding = captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok());

        bindings.push(SourceBinding {
            set: 0,
            binding,
            name,
            order: starting_index + offset,
        });
    }

    bindings.sort_by_key(|b| b.order);

    Ok(bindings)
}

fn take_source_name(set: u32, binding: u32, sources: &mut Vec<SourceBinding>) -> Option<String> {
    if let Some(index) = sources
        .iter()
        .position(|src| src.set == set && src.binding == Some(binding))
    {
        return Some(sources.swap_remove(index).name);
    }

    if let Some(index) = sources
        .iter()
        .position(|src| src.set == set && src.binding.is_none())
    {
        return Some(sources.swap_remove(index).name);
    }

    None
}

fn extract_name(declaration: &str) -> Option<String> {
    let name_regex = Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[\s*\d*\s*\])?\s*$").ok()?;
    let captures = name_regex.captures(declaration)?;
    captures.get(1).map(|m| m.as_str().to_string())
}

fn reflect_metadata(spirv_bytes: &[u8]) -> Result<ShaderMetadata, BentoError> {
    use rspirv_reflect::{Reflection, spirv};

    let reflection = Reflection::new_from_spirv(spirv_bytes)
        .map_err(|e| BentoError::ShaderCompilation(e.to_string()))?;
    let module = &reflection.0;

    let mut names = HashMap::new();
    for instruction in &module.debug_names {
        if instruction.class.opcode == spirv::Op::Name {
            if let (
                Some(rspirv_reflect::rspirv::dr::Operand::IdRef(id)),
                Some(rspirv_reflect::rspirv::dr::Operand::LiteralString(name)),
            ) = (instruction.operands.get(0), instruction.operands.get(1))
            {
                let id = *id;
                names.insert(id, name.clone());
            }
        }
    }

    let mut locations = HashMap::new();
    for instruction in &module.annotations {
        if instruction.class.opcode == spirv::Op::Decorate {
            if let (
                Some(rspirv_reflect::rspirv::dr::Operand::IdRef(id)),
                Some(rspirv_reflect::rspirv::dr::Operand::Decoration(decoration)),
                Some(rspirv_reflect::rspirv::dr::Operand::LiteralBit32(location)),
            ) = (
                instruction.operands.get(0),
                instruction.operands.get(1),
                instruction.operands.get(2),
            ) {
                if *decoration == spirv::Decoration::Location {
                    let id = *id;
                    locations.insert(id, *location);
                }
            }
        }
    }

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut scalar_types = HashMap::new();
    let mut vector_types = HashMap::new();
    let mut pointer_types = HashMap::new();

    for instruction in &module.types_global_values {
        match instruction.class.opcode {
            spirv::Op::TypeFloat => {
                if let (Some(id), Some(rspirv_reflect::rspirv::dr::Operand::LiteralBit32(width))) =
                    (instruction.result_id, instruction.operands.get(0))
                {
                    scalar_types.insert(id, ScalarType::Float(*width));
                }
            }
            spirv::Op::TypeInt => {
                if let (
                    Some(id),
                    Some(rspirv_reflect::rspirv::dr::Operand::LiteralBit32(width)),
                    Some(rspirv_reflect::rspirv::dr::Operand::LiteralBit32(signedness)),
                ) = (
                    instruction.result_id,
                    instruction.operands.get(0),
                    instruction.operands.get(1),
                ) {
                    scalar_types.insert(
                        id,
                        ScalarType::Int {
                            width: *width,
                            signed: *signedness == 1,
                        },
                    );
                }
            }
            spirv::Op::TypeVector => {
                if let (
                    Some(id),
                    Some(rspirv_reflect::rspirv::dr::Operand::IdRef(component_type)),
                    Some(rspirv_reflect::rspirv::dr::Operand::LiteralBit32(component_count)),
                ) = (
                    instruction.result_id,
                    instruction.operands.get(0),
                    instruction.operands.get(1),
                ) {
                    vector_types.insert(
                        id,
                        VectorType {
                            component_type: *component_type,
                            component_count: *component_count,
                        },
                    );
                }
            }
            spirv::Op::TypePointer => {
                if let (Some(id), Some(rspirv_reflect::rspirv::dr::Operand::IdRef(pointee_type))) =
                    (instruction.result_id, instruction.operands.get(1))
                {
                    pointer_types.insert(id, *pointee_type);
                }
            }
            _ => {}
        }
    }

    for instruction in &module.types_global_values {
        if instruction.class.opcode != spirv::Op::Variable {
            continue;
        }

        let Some(id) = instruction.result_id else {
            continue;
        };
        let Some(rspirv_reflect::rspirv::dr::Operand::StorageClass(storage_class)) =
            instruction.operands.get(0)
        else {
            continue;
        };

        let name = names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("var_{id}"));
        let location = locations.get(&id).copied();
        let format = instruction
            .result_type
            .and_then(|ty| pointer_types.get(&ty).copied().or(Some(ty)))
            .and_then(|ty| resolve_primitive(ty, &scalar_types, &vector_types));
        let variable = InterfaceVariable {
            name,
            location,
            format,
        };

        match storage_class {
            spirv::StorageClass::Input => inputs.push(variable),
            spirv::StorageClass::Output => outputs.push(variable),
            _ => {}
        }
    }

    inputs.sort_by(|a, b| {
        a.location
            .cmp(&b.location)
            .then_with(|| a.name.cmp(&b.name))
    });
    outputs.sort_by(|a, b| {
        a.location
            .cmp(&b.location)
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut has_vertex_entry_point = false;
    let entry_points = module
        .entry_points
        .iter()
        .filter_map(|instruction| {
            if instruction.class.opcode != spirv::Op::EntryPoint {
                return None;
            }

            if let Some(rspirv_reflect::rspirv::dr::Operand::ExecutionModel(model)) =
                instruction.operands.get(0)
            {
                if *model == spirv::ExecutionModel::Vertex {
                    has_vertex_entry_point = true;
                }
            }

            match instruction.operands.get(2) {
                Some(rspirv_reflect::rspirv::dr::Operand::LiteralString(name)) => {
                    Some(name.clone())
                }
                _ => None,
            }
        })
        .collect();

    let workgroup_size = reflection
        .get_compute_group_size()
        .map(|(x, y, z)| [x, y, z]);

    let vertex = if has_vertex_entry_point {
        let mut attributes: Vec<(u32, dashi::ShaderPrimitiveType)> = inputs
            .iter()
            .filter_map(|var| var.location.zip(var.format))
            .collect();
        attributes.sort_by_key(|(location, _)| *location);

        let mut offset = 0usize;
        let mut entries = Vec::new();
        for (location, format) in attributes {
            entries.push(VertexEntry {
                format,
                location: location as usize,
                offset,
            });
            offset += primitive_size(format);
        }

        if entries.is_empty() {
            None
        } else {
            Some(VertexLayout {
                entries,
                stride: offset,
                rate: dashi::VertexRate::Vertex,
            })
        }
    } else {
        None
    };

    Ok(ShaderMetadata {
        entry_points,
        inputs,
        outputs,
        workgroup_size,
        vertex,
    })
}

#[derive(Clone, Copy)]
enum ScalarType {
    Float(u32),
    Int { width: u32, signed: bool },
}

#[derive(Clone, Copy)]
struct VectorType {
    component_type: u32,
    component_count: u32,
}

fn resolve_primitive(
    type_id: u32,
    scalars: &HashMap<u32, ScalarType>,
    vectors: &HashMap<u32, VectorType>,
) -> Option<dashi::ShaderPrimitiveType> {
    let vector = vectors.get(&type_id)?;
    let scalar = scalars.get(&vector.component_type)?;

    match (scalar, vector.component_count) {
        (ScalarType::Float(32), 2) => Some(dashi::ShaderPrimitiveType::Vec2),
        (ScalarType::Float(32), 3) => Some(dashi::ShaderPrimitiveType::Vec3),
        (ScalarType::Float(32), 4) => Some(dashi::ShaderPrimitiveType::Vec4),
        (
            ScalarType::Int {
                width: 32,
                signed: true,
            },
            4,
        ) => Some(dashi::ShaderPrimitiveType::IVec4),
        (
            ScalarType::Int {
                width: 32,
                signed: false,
            },
            4,
        ) => Some(dashi::ShaderPrimitiveType::UVec4),
        _ => None,
    }
}

fn primitive_size(format: dashi::ShaderPrimitiveType) -> usize {
    match format {
        dashi::ShaderPrimitiveType::Vec2 => 8,
        dashi::ShaderPrimitiveType::Vec3 => 12,
        dashi::ShaderPrimitiveType::Vec4 => 16,
        dashi::ShaderPrimitiveType::IVec4 => 16,
        dashi::ShaderPrimitiveType::UVec4 => 16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sample_compilation_result() -> CompilationResult {
        CompilationResult {
            name: Some("example".to_string()),
            file: Some("shader.glsl".to_string()),
            lang: ShaderLang::Glsl,
            stage: dashi::ShaderType::Compute,
            variables: vec![ShaderVariable {
                name: "u_time".to_string(),
                set: 0,
                kind: dashi::BindGroupVariable {
                    var_type: dashi::BindGroupVariableType::Uniform,
                    binding: 0,
                    count: 1,
                },
            }],
            metadata: ShaderMetadata {
                entry_points: vec!["main".to_string()],
                inputs: vec![],
                outputs: vec![],
                workgroup_size: Some([1, 1, 1]),
                vertex: None,
            },
            spirv: vec![0x0723_0203, 1, 2, 3],
        }
    }

    #[test]
    fn round_trips_with_binary_serialization() -> Result<(), BentoError> {
        let original = sample_compilation_result();
        let bytes = original.to_bytes()?;
        let restored = CompilationResult::from_bytes(&bytes)?;

        assert_eq!(original, restored);

        Ok(())
    }

    #[test]
    fn saves_and_loads_from_disk() -> Result<(), BentoError> {
        let original = sample_compilation_result();
        let unique_suffix = format!(
            "{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::process::id()
        );
        let path = std::env::temp_dir()
            .join("bento_tests")
            .join(format!("compilation_result_{}.bin", unique_suffix));

        original.save_to_disk(path.to_str().unwrap())?;
        let loaded = CompilationResult::load_from_disk(path.to_str().unwrap())?;

        assert_eq!(original, loaded);

        fs::remove_file(&path).ok();

        Ok(())
    }

    fn sample_request() -> Request {
        Request {
            name: Some("sample".to_string()),
            lang: ShaderLang::Glsl,
            stage: dashi::ShaderType::Compute,
            optimization: OptimizationLevel::None,
            debug_symbols: false,
        }
    }

    fn sample_vertex_request() -> Request {
        Request {
            name: Some("vertex".to_string()),
            lang: ShaderLang::Glsl,
            stage: dashi::ShaderType::Vertex,
            optimization: OptimizationLevel::None,
            debug_symbols: false,
        }
    }

    #[test]
    fn compiles_shader_source() -> Result<(), BentoError> {
        let compiler = Compiler::new()?;
        let shader = include_str!("../tests/fixtures/simple_compute.glsl");
        let request = sample_request();

        let result = compiler.compile(shader.as_bytes(), &request)?;

        assert_eq!(result.name, request.name);
        assert_eq!(result.file, None);
        assert_eq!(result.stage, dashi::ShaderType::Compute);
        assert_eq!(result.lang, ShaderLang::Glsl);
        assert!(!result.spirv.is_empty());
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[0].kind.binding, 0);
        assert_eq!(
            result.variables[0].kind.var_type,
            dashi::BindGroupVariableType::Storage
        );
        assert!(result.metadata.entry_points.contains(&"main".to_string()));
        assert_eq!(result.metadata.workgroup_size, Some([1, 1, 1]));

        Ok(())
    }

    #[test]
    fn compiles_shader_from_file() -> Result<(), BentoError> {
        let compiler = Compiler::new()?;
        let request = sample_request();
        let path = "tests/fixtures/simple_compute.glsl";

        let result = compiler.compile_from_file(path, &request)?;

        assert_eq!(result.file.as_deref(), Some(path));
        assert!(!result.spirv.is_empty());
        assert!(result.metadata.entry_points.contains(&"main".to_string()));

        Ok(())
    }

    #[test]
    fn returns_error_for_missing_file() {
        let compiler = Compiler::new().unwrap();
        let request = sample_request();
        let missing_path = "tests/fixtures/does_not_exist.glsl";

        let err = compiler
            .compile_from_file(missing_path, &request)
            .unwrap_err();

        assert!(matches!(err, BentoError::Io(_)));
    }

    #[test]
    fn returns_error_for_invalid_shader() {
        let compiler = Compiler::new().unwrap();
        let request = sample_request();
        let shader = b"#version 450\nvoid main() {";

        let err = compiler.compile(shader, &request).unwrap_err();

        assert!(matches!(err, BentoError::ShaderCompilation(_)));
    }

    #[test]
    fn reflects_vertex_layout_metadata() -> Result<(), BentoError> {
        let compiler = Compiler::new()?;
        let request = sample_vertex_request();
        let path = "tests/fixtures/simple_vertex.glsl";

        let result = compiler.compile_from_file(path, &request)?;

        let vertex = result
            .metadata
            .vertex
            .expect("expected vertex layout metadata for vertex shader");

        if !matches!(vertex.rate, dashi::VertexRate::Vertex) {
            panic!("expected per-vertex rate");
        }
        assert_eq!(vertex.stride, 20);
        assert_eq!(vertex.entries.len(), 2);

        let first = &vertex.entries[0];
        assert_eq!(first.location, 0);
        assert_eq!(first.offset, 0);
        assert_eq!(first.format, dashi::ShaderPrimitiveType::Vec3);

        let second = &vertex.entries[1];
        assert_eq!(second.location, 1);
        assert_eq!(second.offset, 12);
        assert_eq!(second.format, dashi::ShaderPrimitiveType::Vec2);

        Ok(())
    }
}
