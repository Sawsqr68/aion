//! Boot-side first-run scaffold of the user-level server config.
//!
//! Config LOAD is pure — discovery never creates anything (pinned by
//! `load_home_tests::unconfigured_paths_resolve_under_home_without_eager_creation`).
//! The scaffold therefore lives on the BOOT path: when discovery has found no
//! config at any layer, the server writes `<AION_HOME>/config.toml` from the
//! embedded template and loads again, so a first run boots with the deploy
//! surface and the outbox worker channel commissioned instead of dark.

use tracing::info;

use crate::error::ServerError;

use super::load::LoadedConfig;
use super::{CliOverrides, ConfigSource, ServerConfig, file};

/// The embedded first-run configuration: the exact bytes a first boot with no
/// discovered config writes to `<AION_HOME>/config.toml`.
///
/// Public so the `aion` launcher can recognise a config its spawned server
/// just scaffolded (byte comparison) instead of guessing from existence.
///
/// One truth, two crates: everything from the first section header onward
/// must stay byte-identical to `crates/aion-cli/templates/shared/aion.toml`
/// (the server config `aion new` scaffolds into a project) — only the leading
/// comment block differs, because each file speaks from its own seat. Each
/// crate embeds a copy living inside its own package because an
/// `include_str!` that escapes the crate breaks crates.io publishing (#173);
/// the identity is pinned by
/// `tests::the_embedded_template_matches_the_shared_cli_template`.
pub const FIRST_RUN_CONFIG: &str = include_str!("../../templates/first-run-config.toml");

/// What became of `<AION_HOME>/config.toml` when the scaffold ran.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScaffoldOutcome {
    /// The file was absent and has been written from the embedded template.
    Written,
    /// A file already existed and was left byte-for-byte untouched.
    AlreadyPresent,
}

/// Load the merged server config, scaffolding `<AION_HOME>/config.toml` first
/// when discovery finds no config at any layer.
///
/// `BuiltInDefaults` is the only source that scaffolds: an explicit `--config`
/// naming a missing file has already refused inside [`ServerConfig::load_resolved`],
/// so a typo can never be papered over with a fresh file, and any discovered
/// file — explicit, project-local, or home — is used as-is.
///
/// # Errors
///
/// Returns [`ServerError::Config`] when loading fails or the Aion home cannot
/// be provisioned for the scaffold write.
pub(crate) fn load_or_scaffold(cli: &CliOverrides) -> Result<LoadedConfig, ServerError> {
    let loaded = ServerConfig::load_resolved(cli)?;
    if !matches!(loaded.resolution.source, ConfigSource::BuiltInDefaults) {
        return Ok(loaded);
    }
    scaffold_and_reload(cli, loaded)
}

#[cfg(unix)]
fn scaffold_and_reload(
    cli: &CliOverrides,
    loaded: LoadedConfig,
) -> Result<LoadedConfig, ServerError> {
    // The defaults-only load is spent here: only its resolved home is needed,
    // and the reload below replaces everything else.
    let home = loaded.resolution.home;
    let path = home.join(file::HOME_CONFIG_FILE);
    match scaffold_home_config(&home)? {
        ScaffoldOutcome::Written => info!(
            path = %path.display(),
            "no server config found at any discovery layer; scaffolded a first-run config"
        ),
        ScaffoldOutcome::AlreadyPresent => info!(
            path = %path.display(),
            "a server config appeared after discovery ran; leaving it untouched and using it"
        ),
    }
    ServerConfig::load_resolved(cli)
}

/// On non-Unix targets nothing is scaffolded: the server cannot verify or
/// install a private ACL on the home directory (see
/// [`super::ConfigResolution::ensure_private_home`]), so writing a config into
/// that unverifiable directory would act on exactly the authority the non-Unix
/// path refuses. What the boot does next depends on the home's provenance:
/// with `AION_HOME` explicitly set, `ensure_private_home` treats the home as
/// operator-provisioned and the server boots on built-in defaults — deploy
/// and the outbox stay dark until the operator writes a config; with a
/// derived home, the boot stops at that path's explicit-provisioning refusal.
#[cfg(not(unix))]
fn scaffold_and_reload(
    _cli: &CliOverrides,
    loaded: LoadedConfig,
) -> Result<LoadedConfig, ServerError> {
    Ok(loaded)
}

/// Write `<home>/config.toml` from the embedded template, claiming only what
/// is absent.
///
/// The home directory itself is provisioned owner-only (the same
/// [`crate::filesystem::ConfinedDir`] provisioning `ensure_private_home`
/// performs moments later) and the file lands with owner-only mode. An
/// existing file — including one that appeared between discovery and this
/// write, from a racing boot or the operator's own hand — is never touched:
/// `create_new` refuses it and the refusal is the answer, mirroring the
/// claim-only-when-empty idiom of the embedded assistant install.
#[cfg(unix)]
pub(crate) fn scaffold_home_config(home: &std::path::Path) -> Result<ScaffoldOutcome, ServerError> {
    let dir = crate::filesystem::ConfinedDir::open_or_create(home).map_err(|error| {
        ServerError::Config {
            message: format!("unsafe Aion home `{}`: {error}", home.display()),
        }
    })?;
    match dir.create_new(
        std::path::Path::new(file::HOME_CONFIG_FILE),
        FIRST_RUN_CONFIG.as_bytes(),
    ) {
        Ok(()) => Ok(ScaffoldOutcome::Written),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(ScaffoldOutcome::AlreadyPresent)
        }
        Err(error) => Err(ServerError::Config {
            message: format!(
                "failed to scaffold first-run config `{}`: {error}",
                home.join(file::HOME_CONFIG_FILE).display()
            ),
        }),
    }
}

#[cfg(test)]
#[path = "scaffold_tests.rs"]
mod tests;
