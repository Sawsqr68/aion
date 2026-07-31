//! Resolution of the server's central user-level configuration and state root.

use std::path::PathBuf;

use crate::error::ServerError;

/// Where a resolved Aion home came from.
///
/// This is load-bearing, not diagnostic. Setting `AION_HOME` is how an operator
/// says "this server's state lives here and nowhere else", so every later
/// default that could point state somewhere else has to be able to see that the
/// operator already answered the question. Resolving the home without carrying
/// this fact is what let a working-directory legacy default silently win over
/// an explicitly relocated home.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeSource {
    /// `AION_HOME` was set: the operator named this server's state root.
    Explicit,
    /// `AION_HOME` was absent: the home was derived as `$HOME/.aion`.
    Derived,
}

/// A resolved Aion home together with the provenance of that resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AionHome {
    /// The absolute home directory. Not created: config discovery is read-only.
    pub path: PathBuf,
    /// Whether the operator named the home or it was derived from `$HOME`.
    pub source: HomeSource,
}

/// Resolve the Aion user-level configuration and state directory.
///
/// `AION_HOME` wins when set. Otherwise the directory is `$HOME/.aion`. The
/// returned path is absolute, but is not created: config discovery is read-only,
/// and the store or authoring surface creates it only on first write.
///
/// The returned [`AionHome::source`] records which of those two happened.
/// Callers that choose defaults *below* the home must consult it rather than
/// re-reading `AION_HOME` themselves: the environment is not guaranteed to be
/// the same by then, and a second reading is a second answer.
///
/// # Errors
///
/// Returns [`ServerError::Config`] when `AION_HOME` is empty, neither
/// `AION_HOME` nor `HOME` can identify a home directory, or a relative
/// `AION_HOME` cannot be resolved because the current directory is unavailable.
pub fn aion_home() -> Result<AionHome, ServerError> {
    let configured = std::env::var_os("AION_HOME");
    let (path, source) = match configured {
        Some(value) if value.is_empty() => {
            return Err(ServerError::Config {
                message: "AION_HOME must not be empty".to_owned(),
            });
        }
        Some(value) => (PathBuf::from(value), HomeSource::Explicit),
        None => {
            let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) else {
                return Err(ServerError::Config {
                    message: "cannot resolve Aion home: set AION_HOME or HOME".to_owned(),
                });
            };
            (PathBuf::from(home).join(".aion"), HomeSource::Derived)
        }
    };
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| ServerError::Config {
                message: format!(
                    "cannot resolve relative AION_HOME against the current directory: {source}"
                ),
            })?
    };
    Ok(AionHome { path, source })
}
