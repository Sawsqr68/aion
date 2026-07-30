//! Library surface of the staged-rounds worker: the shell-activity wire
//! types, the typed git/command shell boundary, the embedded agent output
//! schemas, the startup-loaded role profiles, the per-role prompt assembly,
//! the profile-injecting harness wrapper, the reality-wins agent-output
//! rewrites, and the six shell-activity handler bodies.
//!
//! The binary (`main.rs`) is the composition root: it groups the FIVE driven
//! agent roles (planner, developer, reviewer, remediator, and the integration
//! fixer — which shares the remediator's node and session) into one connection
//! per node, installs a `ProfiledNornHarness` on each, and registers the shell
//! handlers with the `aion-worker` SDK. The hermetic tests drive the same
//! handler functions directly.

pub mod agent_output;
pub mod commit;
pub mod handlers;
pub mod harness;
pub mod lens;
pub mod paths;
pub mod profiles;
pub mod prompts;
pub mod schemas;
pub mod shell;
pub mod types;
