//! Shared direct AWL compile-and-assemble seam.

use std::path::Path;

use aion_awl::{CompileError, CompiledWorkflow};

use crate::source::{AwlSourceError, collect_awl_source};
use crate::{AssembleError, AwlAssembleOptions, assemble_awl};

/// Direct-compiled workflow metadata and its complete `.aion` archive.
#[derive(Debug)]
pub struct PreparedAwlPackage {
    /// Compiler output carrying document-derived identity and contracts.
    pub compiled: CompiledWorkflow,
    /// Complete deterministic archive ready for the deploy loader.
    pub archive: Vec<u8>,
}

/// A refusal from [`compile_and_assemble_awl`].
#[derive(Debug, thiserror::Error)]
pub enum PrepareAwlError {
    /// The direct compiler refused the document.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// The document's own source could not be collected for archiving.
    #[error(transparent)]
    Source(#[from] AwlSourceError),
    /// Native package assembly refused the compiler output.
    #[error(transparent)]
    Assemble(#[from] AssembleError),
}

/// Compiles AWL source with the direct bytecode compiler and assembles the
/// result with the document's declared workflow timeout and its own source.
///
/// This is the shared native seam used by CLI and server deploy paths. It does
/// not invoke the legacy Gleam emitter or an external toolchain.
///
/// `document_filename` is the document's own filename, extension included,
/// with no directory part: the archive's `awl/` tree is rooted at
/// `schema_root` (the document's directory), and the document sits at the root
/// of it. `schema_root` is both where `schema(…)` imports resolve and where
/// they are read from for archiving, so a deployed package always carries the
/// exact bytes that were compiled.
///
/// # Errors
///
/// Returns [`PrepareAwlError::Compile`] with the direct compiler's diagnostic
/// unchanged, [`PrepareAwlError::Source`] when the document's filename or an
/// imported schema cannot be archived, or [`PrepareAwlError::Assemble`] when
/// archive assembly refuses the compiled output.
pub fn compile_and_assemble_awl(
    source: &str,
    schema_root: &Path,
    document_filename: &str,
) -> Result<PreparedAwlPackage, PrepareAwlError> {
    let compiled = aion_awl::compile(source, schema_root)?;
    let archived_source = collect_awl_source(document_filename, source, schema_root)?;
    let archive = assemble_awl(
        &compiled,
        AwlAssembleOptions {
            timeout: compiled.timeout,
            source: Some(archived_source),
        },
    )?;
    Ok(PreparedAwlPackage { compiled, archive })
}
