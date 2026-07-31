//! Focused regressions for Aion-home config discovery and state roots.

use std::{fs, path::Path, process::Command};

use crate::config::ConfigSource;
use crate::test_support::CapturedLogs;

use super::{CliOverrides, HomeSource, ServerConfig, aion_home};

const HOME_PROBE: &str = "AION_HOME_TEST_PROBE";
const HOME_EXPECTED: &str = "AION_HOME_TEST_EXPECTED";

fn write(path: &Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

#[test]
fn discovery_precedence_covers_explicit_local_home_and_defaults()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("home");
    let working_dir = scratch.path().join("project");
    fs::create_dir_all(&working_dir)?;
    let home_config = home.join("config.toml");
    let local_config = working_dir.join("aion.toml");
    let explicit_config = scratch.path().join("explicit.toml");

    write(&home_config, "[namespaces]\ndefault = \"home\"\n")?;
    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(loaded.config.namespaces.default, "home");
    assert_eq!(
        loaded.resolution.source,
        ConfigSource::AionHome(home_config)
    );

    write(&local_config, "[namespaces]\ndefault = \"local\"\n")?;
    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(loaded.config.namespaces.default, "local");
    assert_eq!(
        loaded.resolution.source,
        ConfigSource::ProjectLocal(local_config.clone())
    );

    write(&explicit_config, "[namespaces]\ndefault = \"explicit\"\n")?;
    let cli = CliOverrides {
        config_path: Some(explicit_config.clone()),
        ..CliOverrides::default()
    };
    let loaded = ServerConfig::load_for_test(&cli, &home, HomeSource::Derived, &working_dir)?;
    assert_eq!(loaded.config.namespaces.default, "explicit");
    assert_eq!(
        loaded.resolution.source,
        ConfigSource::Explicit(explicit_config)
    );

    fs::remove_file(local_config)?;
    fs::remove_file(home.join("config.toml"))?;
    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(loaded.config.namespaces.default, "default");
    assert_eq!(loaded.resolution.source, ConfigSource::BuiltInDefaults);
    Ok(())
}

#[test]
fn malformed_home_config_is_a_loud_typed_failure() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("home");
    let working_dir = scratch.path().join("project");
    fs::create_dir_all(&working_dir)?;
    let config_path = home.join("config.toml");
    write(&config_path, "[store\nbackend = nope")?;

    let error = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )
    .err()
    .ok_or("malformed home config unexpectedly loaded")?;
    let message = error.to_string();
    assert!(message.contains("failed to parse Aion home file"));
    assert!(message.contains(&config_path.display().to_string()));
    Ok(())
}

#[test]
fn unconfigured_paths_resolve_under_home_without_eager_creation()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("not-created-yet");
    let working_dir = scratch.path().join("project");
    fs::create_dir_all(&working_dir)?;

    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        home.join("data").to_str()
    );
    assert_eq!(
        loaded.config.authoring.workspace_dir.as_deref(),
        Some(home.join("authoring").as_path())
    );
    assert!(
        !home.exists(),
        "config reads must not eagerly create Aion home"
    );
    Ok(())
}

/// The DERIVED world: no `AION_HOME`, so the operator has moved nothing and
/// their durable state genuinely lives in the working directory. The guard
/// adopts it and says so. A fix to #113 that broke this would strand real
/// state, which is worse than the defect it was fixing.
#[test]
fn a_derived_home_adopts_legacy_directories_and_guards_only_unconfigured_defaults()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("home");
    let working_dir = scratch.path().join("project");
    let legacy_data = working_dir.join("aion-data");
    let legacy_authoring = working_dir.join("aion-authoring");
    fs::create_dir_all(&legacy_data)?;
    fs::create_dir_all(&legacy_authoring)?;

    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        legacy_data.to_str()
    );
    assert_eq!(
        loaded.config.authoring.workspace_dir.as_deref(),
        Some(legacy_authoring.as_path())
    );
    assert_eq!(loaded.resolution.legacy_notices.len(), 2);
    for notice in &loaded.resolution.legacy_notices {
        let line = notice.to_string();
        assert!(line.contains("AION HOME MIGRATION REQUIRED"));
        assert!(line.contains("stop the server, move"));
        assert!(line.contains(&home.display().to_string()));
        assert!(line.contains(&working_dir.display().to_string()));
    }

    let (captured, ()) = CapturedLogs::capture(|| loaded.resolution.log_startup());
    let logs = captured.text()?;
    assert!(logs.contains("Aion home legacy-directory migration guard active"));
    assert!(logs.contains("AION HOME MIGRATION REQUIRED"));
    assert!(logs.contains(&legacy_data.display().to_string()));
    assert!(logs.contains(&home.join("data").display().to_string()));
    assert!(logs.contains("aion-server configuration resolved"));
    assert!(logs.contains("built-in defaults"));
    assert!(logs.contains("aion-server data root resolved"));
    assert!(logs.contains("aion-server authoring root resolved"));

    write(
        &working_dir.join("aion.toml"),
        "[store]\ndata_dir = \"configured-data\"\n\n[authoring]\nworkspace_dir = \"configured-authoring\"\n",
    )?;
    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        Some("configured-data")
    );
    assert_eq!(
        loaded.config.authoring.workspace_dir.as_deref(),
        Some(Path::new("configured-authoring"))
    );
    assert!(loaded.resolution.legacy_notices.is_empty());
    Ok(())
}

#[cfg(unix)]
#[test]
fn legacy_symlink_directories_are_never_selected() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("home");
    let working_dir = scratch.path().join("project");
    let outside = scratch.path().join("outside");
    fs::create_dir(&working_dir)?;
    fs::create_dir(&outside)?;
    symlink(&outside, working_dir.join("aion-data"))?;
    symlink(&outside, working_dir.join("aion-authoring"))?;

    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Derived,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        home.join("data").to_str()
    );
    assert_eq!(
        loaded.config.authoring.workspace_dir.as_deref(),
        Some(home.join("authoring").as_path())
    );
    assert!(loaded.resolution.legacy_notices.is_empty());
    Ok(())
}

/// The EXPLICIT world, and the #113 regression pin: an operator who names a
/// home is isolating this server's state, so a legacy directory sitting in the
/// working directory must not be able to drag it back. **This assertion fails
/// against the code as it stood before the fix** — both roots resolved to the
/// working directory instead.
#[test]
fn an_explicit_home_is_never_overridden_by_a_legacy_working_directory()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("isolated-home");
    let working_dir = scratch.path().join("project");
    let legacy_data = working_dir.join("aion-data");
    let legacy_authoring = working_dir.join("aion-authoring");
    fs::create_dir_all(&legacy_data)?;
    fs::create_dir_all(&legacy_authoring)?;

    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Explicit,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        home.join("data").to_str(),
        "an explicitly named home lost the data root to a legacy working directory"
    );
    assert_eq!(
        loaded.config.authoring.workspace_dir.as_deref(),
        Some(home.join("authoring").as_path()),
        "an explicitly named home lost the authoring root to a legacy working directory"
    );

    // Ignoring it silently would be its own defect: the operator's control was
    // honoured, and they still need told the other directory is sitting there
    // and that nothing is reading or writing it.
    assert_eq!(loaded.resolution.legacy_notices.len(), 2);
    for notice in &loaded.resolution.legacy_notices {
        let line = notice.to_string();
        assert!(
            line.contains("AION HOME IS SET, SO THE LEGACY DIRECTORY WAS IGNORED"),
            "an ignored legacy directory was reported as a migration chore: {line}"
        );
        assert!(!line.contains("MIGRATION REQUIRED"));
        assert!(line.contains(&working_dir.display().to_string()));
        assert!(line.contains(&home.display().to_string()));
    }

    let (captured, ()) = CapturedLogs::capture(|| loaded.resolution.log_startup());
    let logs = captured.text()?;
    assert!(logs.contains("Aion home legacy directory ignored because AION_HOME is set"));
    assert!(!logs.contains("migration guard active"));
    assert!(logs.contains(&home.join("data").display().to_string()));
    Ok(())
}

/// The control against fixing #113 by disabling the branch outright: with no
/// legacy directory present there is nothing to report, so a suite that only
/// asserted "notices exist" would pass on a server that reported them always.
#[test]
fn an_explicit_home_with_no_legacy_directory_reports_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("isolated-home");
    let working_dir = scratch.path().join("project");
    fs::create_dir_all(&working_dir)?;

    let loaded = ServerConfig::load_for_test(
        &CliOverrides::default(),
        &home,
        HomeSource::Explicit,
        &working_dir,
    )?;
    assert_eq!(
        loaded.config.store.data_dir.as_deref(),
        home.join("data").to_str()
    );
    assert!(loaded.resolution.legacy_notices.is_empty());
    Ok(())
}

#[test]
fn aion_home_environment_override_is_respected_everywhere() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = crate::test_support::private_tempdir()?;
    let home = scratch.path().join("overridden-home");
    let working_dir = scratch.path().join("project");
    fs::create_dir_all(&working_dir)?;
    // #113 regression, planted at the production path: the child runs with a
    // real `AION_HOME` and a working directory that holds both legacy
    // directories. Before the fix these won, and a server the operator had
    // deliberately isolated wrote its durable state into the shared location it
    // was pointed away from. The child asserts both roots resolve under the
    // home, so this is a live inversion pin and not a log-message check.
    fs::create_dir_all(working_dir.join("aion-data"))?;
    fs::create_dir_all(working_dir.join("aion-authoring"))?;
    write(
        &home.join("config.toml"),
        "[namespaces]\ndefault = \"from-home-config\"\n",
    )?;
    let executable = std::env::current_exe()?;
    let status = Command::new(executable)
        .arg("--exact")
        .arg("config::load::home_tests::aion_home_env_probe_child")
        .arg("--nocapture")
        .current_dir(&working_dir)
        .env("AION_HOME", &home)
        .env(HOME_PROBE, "1")
        .env(HOME_EXPECTED, &home)
        .env_remove("AION_STORE_DATA_DIR")
        .env_remove("AION_AUTHORING_WORKSPACE_DIR")
        .status()?;
    assert!(status.success(), "AION_HOME probe child failed");
    Ok(())
}

#[test]
fn missing_home_is_a_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let status = Command::new(executable)
        .arg("--exact")
        .arg("config::load::home_tests::aion_home_env_probe_child")
        .arg("--nocapture")
        .env_remove("AION_HOME")
        .env_remove("HOME")
        .env(HOME_PROBE, "missing")
        .status()?;
    assert!(status.success(), "missing-home probe child failed");
    Ok(())
}

#[test]
fn aion_home_env_probe_child() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mode) = std::env::var_os(HOME_PROBE) else {
        return Ok(());
    };
    if mode == "missing" {
        let error = aion_home()
            .err()
            .ok_or("missing AION_HOME and HOME unexpectedly resolved")?;
        assert!(error.to_string().contains("set AION_HOME or HOME"));
        return Ok(());
    }
    let expected = std::env::var_os(HOME_EXPECTED).ok_or("probe omitted expected home")?;
    let expected = std::path::PathBuf::from(expected);
    let resolved = aion_home()?;
    assert_eq!(resolved.path, expected);
    // The provenance is the fact the legacy fallback needs and used to lose.
    assert_eq!(resolved.source, HomeSource::Explicit);
    let config = ServerConfig::load(&CliOverrides::default())?;
    assert_eq!(config.namespaces.default, "from-home-config");
    assert_eq!(
        config.store.data_dir.as_deref(),
        expected.join("data").to_str()
    );
    assert_eq!(
        config.authoring.workspace_dir.as_deref(),
        Some(expected.join("authoring").as_path())
    );
    Ok(())
}
