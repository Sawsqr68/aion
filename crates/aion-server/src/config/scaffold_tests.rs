//! Regressions for the boot-side first-run config scaffold.

use super::FIRST_RUN_CONFIG;

/// The two embedded copies of the shared server-config template — this
/// crate's scaffold source and aion-cli's `aion new` template — must stay
/// byte-identical from the first section header (`[server]`) onward. Only the
/// leading comment block may differ, because each file speaks truthfully from
/// its own seat (one is written by the server into the home, one by `aion new`
/// into a project). Each crate embeds a file inside its own package because an
/// `include_str!` escaping the crate breaks crates.io publishing (#173); this
/// pin is what makes the pair one truth instead of two drifting copies.
#[test]
fn the_embedded_template_matches_the_shared_cli_template() -> Result<(), Box<dyn std::error::Error>>
{
    let shared = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aion-cli/templates/shared/aion.toml"
    ))?;
    let embedded_body = template_body(FIRST_RUN_CONFIG)?;
    let shared_body = template_body(&shared)?;
    assert_eq!(
        embedded_body, shared_body,
        "crates/aion-server/templates/first-run-config.toml and \
         crates/aion-cli/templates/shared/aion.toml have drifted apart from \
         `[server]` onward; edit one body and copy it over the other \
         (only the leading comment block may differ)"
    );
    Ok(())
}

/// Everything from the first `[server]` section header onward.
fn template_body(template: &str) -> Result<&str, Box<dyn std::error::Error>> {
    let start = template
        .find("[server]")
        .ok_or("template has no [server] section")?;
    Ok(&template[start..])
}

/// #180 review MAJ-3: the launcher probes the address the BUILT-IN DEFAULTS
/// resolve while a first run boots on the SCAFFOLDED template — that only
/// works because the two answers coincide. Pin the coincidence: a template
/// port edit must fail here, not strand a newcomer's first `aion` probing a
/// port the server never bound.
#[test]
fn the_template_addresses_equal_the_built_in_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let parsed: toml::Value = toml::from_str(FIRST_RUN_CONFIG)?;
    let server = parsed
        .get("server")
        .ok_or("template has no [server] section")?;
    let listen: std::net::SocketAddr = server
        .get("listen_address")
        .and_then(toml::Value::as_str)
        .ok_or("template has no server.listen_address string")?
        .parse()?;
    let grpc: std::net::SocketAddr = server
        .get("grpc_address")
        .and_then(toml::Value::as_str)
        .ok_or("template has no server.grpc_address string")?
        .parse()?;
    assert_eq!(
        listen,
        super::super::defaults::DEFAULT_HTTP_ADDRESS,
        "the template's server.listen_address must equal the built-in default: \
         the bare `aion` launcher resolves the default address before the \
         scaffolded config exists and then waits on it"
    );
    assert_eq!(
        grpc,
        super::super::defaults::DEFAULT_GRPC_ADDRESS,
        "the template's server.grpc_address must equal the built-in default"
    );
    Ok(())
}

#[cfg(unix)]
mod unix {
    use std::fs;

    use crate::config::{CliOverrides, ConfigSource, HomeSource, ServerConfig};

    use super::super::{FIRST_RUN_CONFIG, ScaffoldOutcome, scaffold_home_config};

    /// First start on a clean machine: the scaffold provisions the home,
    /// writes the template owner-only, and the very next discovery pass finds
    /// it as the Aion-home layer with the deploy surface and the outbox
    /// worker channel commissioned — the two surfaces that are dark on
    /// built-in defaults.
    #[test]
    fn a_missing_home_config_is_scaffolded_owner_only_and_lights_deploy_and_outbox()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = crate::test_support::private_tempdir()?;
        let home = scratch.path().join(".aion");
        let working_dir = scratch.path().join("project");
        fs::create_dir_all(&working_dir)?;

        let outcome = scaffold_home_config(&home)?;
        assert_eq!(outcome, ScaffoldOutcome::Written);

        let config_path = home.join("config.toml");
        assert_eq!(
            fs::metadata(&config_path)?.permissions().mode() & 0o777,
            0o600,
            "the scaffolded config must be owner-only"
        );
        assert_eq!(fs::metadata(&home)?.permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::read_to_string(&config_path)?, FIRST_RUN_CONFIG);

        let loaded = ServerConfig::load_for_test(
            &CliOverrides::default(),
            &home,
            HomeSource::Derived,
            &working_dir,
        )?;
        assert_eq!(
            loaded.resolution.source,
            ConfigSource::AionHome(config_path)
        );
        assert!(loaded.config.deploy.enabled, "deploy must be commissioned");
        assert!(loaded.config.outbox.enabled, "outbox must be commissioned");
        assert_eq!(
            loaded.config.outbox.liminal_listen_address.as_deref(),
            Some("127.0.0.1:50061")
        );
        // The template must not name a data_dir: the home default applies,
        // and the historical literal `aion-data` is the legacy working-dir
        // path that re-activates the migration guard.
        assert_eq!(
            loaded.config.store.data_dir.as_deref(),
            home.join("data").to_str()
        );
        Ok(())
    }

    /// Claim only what is absent: an existing config — whatever it says, in
    /// whatever state — is the operator's, byte for byte.
    #[test]
    fn an_existing_home_config_is_left_byte_for_byte_untouched()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::private_tempdir()?;
        let home = scratch.path().join(".aion");
        fs::create_dir_all(&home)?;
        let config_path = home.join("config.toml");
        let operators_own = b"# hand-written\n[namespaces]\ndefault = \"kept\"\n";
        fs::write(&config_path, operators_own)?;

        let outcome = scaffold_home_config(&home)?;
        assert_eq!(outcome, ScaffoldOutcome::AlreadyPresent);
        assert_eq!(
            fs::read(&config_path)?,
            operators_own.to_vec(),
            "an existing config was rewritten"
        );
        Ok(())
    }
}
