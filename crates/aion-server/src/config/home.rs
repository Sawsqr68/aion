//! Resolution of the server's central user-level configuration and state root.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

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
/// `AION_HOME` nor `HOME` can identify a home directory, a `~`-prefixed
/// `AION_HOME` cannot be expanded (see [`expand_tilde`]), or a relative
/// `AION_HOME` cannot be resolved because the current directory is unavailable.
pub fn aion_home() -> Result<AionHome, ServerError> {
    let configured = std::env::var_os("AION_HOME");
    let (path, source) = match configured {
        Some(value) if value.is_empty() => {
            return Err(ServerError::Config {
                message: "AION_HOME must not be empty".to_owned(),
            });
        }
        Some(value) => (expand_tilde(Path::new(&value))?, HomeSource::Explicit),
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

/// Expand a leading `~` in an operator-supplied path against `$HOME`.
///
/// `~` alone and `~/rest` expand (#152: `AION_HOME=~/.aion` and
/// `--config ~/aion.toml` reach the loader unexpanded whenever a shell did not
/// get to them — a quoted value, an env file, a process manager). Any other
/// `~`-leading form (`~user/...`) is a loud typed refusal: expanding it needs
/// the user database, and silently treating it as a relative directory
/// literally named `~user` is exactly the quiet misdirection this fixes. Every
/// other path passes through untouched.
pub(super) fn expand_tilde(path: &Path) -> Result<PathBuf, ServerError> {
    expand_tilde_against(path, std::env::var_os("HOME"))
}

fn expand_tilde_against(path: &Path, home: Option<OsString>) -> Result<PathBuf, ServerError> {
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return Ok(path.to_owned());
    };
    if first == "~" {
        let Some(home) = home.filter(|value| !value.is_empty()) else {
            return Err(ServerError::Config {
                message: format!(
                    "cannot expand `~` in path `{}`: HOME is not set",
                    path.display()
                ),
            });
        };
        let rest = components.as_path();
        return Ok(if rest.as_os_str().is_empty() {
            PathBuf::from(home)
        } else {
            PathBuf::from(home).join(rest)
        });
    }
    if first.as_encoded_bytes().starts_with(b"~") {
        return Err(ServerError::Config {
            message: format!(
                "cannot resolve `{}`: `~user` expansion is not supported; use an absolute \
                 path (a directory literally named `{}` can be spelled `./{}`)",
                path.display(),
                Path::new(first).display(),
                Path::new(first).display()
            ),
        });
    }
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> OsString {
        OsString::from("/users/probe")
    }

    #[test]
    fn a_bare_tilde_expands_to_home() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            expand_tilde_against(Path::new("~"), Some(home()))?,
            PathBuf::from("/users/probe")
        );
        Ok(())
    }

    #[test]
    fn a_tilde_slash_prefix_expands_under_home() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            expand_tilde_against(Path::new("~/.aion/config.toml"), Some(home()))?,
            PathBuf::from("/users/probe/.aion/config.toml")
        );
        Ok(())
    }

    #[test]
    fn non_tilde_paths_pass_through_untouched() -> Result<(), Box<dyn std::error::Error>> {
        for literal in ["/absolute/path", "relative/path", "./~escaped", ""] {
            assert_eq!(
                expand_tilde_against(Path::new(literal), Some(home()))?,
                PathBuf::from(literal),
                "`{literal}` must not be rewritten"
            );
        }
        Ok(())
    }

    /// A path whose SECOND component is a tilde is a literal name, not an
    /// expansion site — only a leading tilde means "my home".
    #[test]
    fn an_interior_tilde_is_a_literal_name() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            expand_tilde_against(Path::new("/srv/~backup"), Some(home()))?,
            PathBuf::from("/srv/~backup")
        );
        Ok(())
    }

    #[test]
    fn a_tilde_user_form_is_a_loud_typed_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let error = expand_tilde_against(Path::new("~alice/.aion"), Some(home()))
            .err()
            .ok_or("`~alice` was accepted")?;
        let message = error.to_string();
        assert!(message.contains("~user"));
        assert!(message.contains("~alice"));
        assert!(message.contains("./~alice"));
        Ok(())
    }

    #[test]
    fn a_tilde_without_home_is_a_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        for absent in [None, Some(OsString::new())] {
            let error = expand_tilde_against(Path::new("~/.aion"), absent)
                .err()
                .ok_or("`~` expanded without HOME")?;
            assert!(error.to_string().contains("HOME is not set"));
        }
        Ok(())
    }
}
