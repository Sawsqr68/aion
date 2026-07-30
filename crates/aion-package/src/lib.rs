//! Archive validation, content hashing, and namespacing for Aion packages.
//!
//! This crate loads `.aion` archives, validates manifests and BEAM entries,
//! computes stable content hashes, and derives deployed module names for engine
//! registration.
//!
//! # Example
//!
//! ```no_run
//! use aion_package::{ExtractionLimits, Package};
//!
//! // Operator-local file: extraction may run unbounded. Network input must
//! // use `ExtractionLimits::bounded` instead.
//! let package = Package::load_from_path("workflow.aion", ExtractionLimits::unbounded())?;
//! println!("entry module: {}", package.manifest().entry_module);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

/// Declared-contract admission of caller-supplied values at a boundary.
pub mod admission;
/// Compiled BEAM module records extracted from packages.
pub mod beam;
/// Archive builder utilities for tests and packaging tools.
pub mod builder;
/// Canonical JSON text encoding for emitted artifacts.
pub mod canonical;
/// Gleam type and JSON codec generation from project schemas.
pub mod codegen;
/// Worker advertisement compatibility and field-level differences.
pub mod compatibility;
/// Durable worker-contract records and canonical identity encoding.
pub mod contract;
/// Package validation and archive-loading errors.
pub mod error;
/// Explicit inflate budgets for archive extraction.
pub mod extraction;
/// Stable content-hash calculation for package contents.
pub mod hash;
/// Manifest structures and format-version constants.
pub mod manifest;
/// Deployment namespace helpers for compiled module names.
pub mod namespace;
/// Validated in-memory package loading and accessors.
pub mod package;
/// Project-level packaging driven by `workflow.toml` descriptors.
pub mod project;
/// Workflow primitive-structure projection: the graph model and bounded
/// Gleam regeneration (a projection of the typed source, never authoritative).
pub mod structure;
/// Workflow version identifiers derived from package content.
pub mod version;

pub use admission::{AdmissionError, admit_value, declares_nothing};
pub use beam::{BeamModule, BeamSet, RESERVED_MODULE_NAMES};
pub use builder::PackageBuilder;
pub use canonical::{CanonicalJson, serialize_value, sorted_entries};
pub use codegen::{
    ActivityArtifact, ActivityDeclaration, ActivityReport, AionDependency, AwlScaffoldError,
    AwlWorkerScaffold, BoundaryType, CodecReport, CodegenError, CodegenMode, Connection,
    ConnectionPlan, DocumentRoot, FileOwnership, ScaffoldedFile, SchemaEmitReport, ServableAction,
    TestScaffoldReport, Tier, WorkerScaffold, boundary_types_from_interface, build_input_skeleton,
    emit_schemas, generate_activities, generate_codecs, generate_test_scaffold, parse_declarations,
    scaffold_awl_worker,
};
pub use compatibility::{ContractDiff, contract_diffs};
pub use contract::{
    ActionBodyContract, ActionContract, ActivityDescriptor, AdditionalWorkflowContract,
    ChildContract, ContractIdentityError, PackageContract, RetryContract, SignalContract,
    WorkerContract,
};
pub use error::PackageError;
pub use extraction::ExtractionLimits;
pub use hash::{
    ContentHash, content_hash, content_hash_with_contract, content_hash_with_timeout,
    content_hash_with_timeouts,
};
pub use manifest::{
    CURRENT_FORMAT_VERSION, DeclaredActivity, Manifest, ManifestDigest, ManifestVersion,
    WorkflowEntry,
};
pub use namespace::{
    DEPLOYED_NAME_SEPARATOR, NamespaceError, ParsedDeployedName, deployed_name, deployed_names,
    parse_deployed_name,
};
pub use package::Package;
pub use project::{
    ExcludedModule, ExcludedReason, PackageOptions, PackagedWorkflow, PackagingError,
    ProjectReport, package_project,
};
pub use structure::{
    ArmLabel, CorrelationKey, DeterminismError, EdgeKind, FactsError, GraphEdge, GraphNode, NodeId,
    NodePrimitive, StructuralDelta, StructureError, Violation, ViolationKind, WorkflowFacts,
    WorkflowGraph, analyze_determinism, extract_structure, extract_workflow_facts,
    regenerate_gleam,
};
pub use version::WorkflowVersion;
