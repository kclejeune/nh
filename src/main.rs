mod checks;
mod clean;
mod commands;
mod darwin;
mod generations;
mod home;
mod installable;
mod interface;
mod json;
mod logging;
mod nixos;
mod search;
mod system;
mod update;
mod util;

use clap::{CommandFactory, FromArgMatches};
use color_eyre::Result;
#[cfg(feature = "hotpath")] use hotpath;

use crate::{commands::ElevationStrategy, interface::SystemType};

pub const NH_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NH_REV: Option<&str> = option_env!("NH_REV");

/// Resolve system type from CLI args or environment variable (for early
/// parsing)
fn resolve_system_type_early() -> Option<SystemType> {
  // First check CLI args, then fall back to env var
  SystemType::from_args().or_else(|| SystemType::from_env().ok())
}

/// Modify the CLI command to hide system subcommands incompatible with
/// `--system-type` or `NH_SYSTEM_TYPE`
fn configure_system_subcommands(cmd: clap::Command) -> clap::Command {
  cmd.mut_subcommand("system", |system_cmd| {
    match resolve_system_type_early() {
      Some(SystemType::Os) => system_cmd, // All subcommands visible for NixOS
      Some(SystemType::Darwin) => {
        // Darwin doesn't support boot or test
        system_cmd
          .mut_subcommand("boot", |c| c.hide(true))
          .mut_subcommand("test", |c| c.hide(true))
      },
      Some(SystemType::Home) => {
        // Home doesn't support boot or test
        system_cmd
          .mut_subcommand("boot", |c| c.hide(true))
          .mut_subcommand("test", |c| c.hide(true))
      },
      None => system_cmd, // Show all if not specified
    }
  })
}

fn main() -> Result<()> {
  #[cfg(feature = "hotpath")]
  let _guard = hotpath::GuardBuilder::new("main").build();

  // Build the CLI command and configure system subcommands based on
  // NH_SYSTEM_TYPE
  let cmd = configure_system_subcommands(crate::interface::Main::command());
  let matches = cmd.get_matches();
  let args = crate::interface::Main::from_arg_matches(&matches)?;

  // Set up logging
  crate::logging::setup_logging(args.verbosity)?;
  tracing::debug!("{args:#?}");
  tracing::debug!(%NH_VERSION, ?NH_REV);

  // Check Nix version upfront
  checks::verify_nix_environment()?;

  // Once we assert required Nix features, validate NH environment checks
  // For now, this is just NH_* variables being set. More checks may be
  // added to setup_environment in the future.
  checks::verify_variables()?;

  let elevation = args
    .elevation_program
    .map_or(ElevationStrategy::Auto, ElevationStrategy::Prefer);

  args.command.run(elevation)
}

#[cfg(test)]
mod tests {
  use std::env;

  use clap::CommandFactory;
  use serial_test::serial;

  use super::*;

  // Helper to safely set/restore environment variables in tests
  struct EnvGuard {
    key:      String,
    original: Option<String>,
  }

  impl EnvGuard {
    fn new(key: &str, value: &str) -> Self {
      let original = env::var(key).ok();
      unsafe {
        env::set_var(key, value);
      }
      EnvGuard {
        key: key.to_string(),
        original,
      }
    }

    fn remove(key: &str) -> Self {
      let original = env::var(key).ok();
      unsafe {
        env::remove_var(key);
      }
      EnvGuard {
        key: key.to_string(),
        original,
      }
    }
  }

  impl Drop for EnvGuard {
    fn drop(&mut self) {
      unsafe {
        match &self.original {
          Some(val) => env::set_var(&self.key, val),
          None => env::remove_var(&self.key),
        }
      }
    }
  }

  /// Helper to check if a subcommand is hidden within the system command
  fn is_system_subcommand_hidden(
    cmd: &clap::Command,
    subcommand_name: &str,
  ) -> bool {
    cmd
      .get_subcommands()
      .find(|c| c.get_name() == "system")
      .and_then(|system_cmd| {
        system_cmd
          .get_subcommands()
          .find(|c| c.get_name() == subcommand_name)
      })
      .map(|subcmd| subcmd.is_hide_set())
      .unwrap_or(false)
  }

  #[test]
  #[serial]
  fn test_configure_system_subcommands_without_env_shows_all() {
    let _guard = EnvGuard::remove("NH_SYSTEM_TYPE");
    let cmd = configure_system_subcommands(crate::interface::Main::command());

    assert!(
      !is_system_subcommand_hidden(&cmd, "switch"),
      "switch should be visible"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "build"),
      "build should be visible"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "boot"),
      "boot should be visible when NH_SYSTEM_TYPE is not set"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "test"),
      "test should be visible when NH_SYSTEM_TYPE is not set"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "repl"),
      "repl should be visible"
    );
  }

  #[test]
  #[serial]
  fn test_configure_system_subcommands_os_shows_all() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "os");
    let cmd = configure_system_subcommands(crate::interface::Main::command());

    assert!(
      !is_system_subcommand_hidden(&cmd, "switch"),
      "switch should be visible for os"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "build"),
      "build should be visible for os"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "boot"),
      "boot should be visible for os"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "test"),
      "test should be visible for os"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "repl"),
      "repl should be visible for os"
    );
  }

  #[test]
  #[serial]
  fn test_configure_system_subcommands_darwin_hides_boot_test() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "darwin");
    let cmd = configure_system_subcommands(crate::interface::Main::command());

    assert!(
      !is_system_subcommand_hidden(&cmd, "switch"),
      "switch should be visible for darwin"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "build"),
      "build should be visible for darwin"
    );
    assert!(
      is_system_subcommand_hidden(&cmd, "boot"),
      "boot should be hidden for darwin"
    );
    assert!(
      is_system_subcommand_hidden(&cmd, "test"),
      "test should be hidden for darwin"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "repl"),
      "repl should be visible for darwin"
    );
  }

  #[test]
  #[serial]
  fn test_configure_system_subcommands_home_hides_boot_test() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "home");
    let cmd = configure_system_subcommands(crate::interface::Main::command());

    assert!(
      !is_system_subcommand_hidden(&cmd, "switch"),
      "switch should be visible for home"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "build"),
      "build should be visible for home"
    );
    assert!(
      is_system_subcommand_hidden(&cmd, "boot"),
      "boot should be hidden for home"
    );
    assert!(
      is_system_subcommand_hidden(&cmd, "test"),
      "test should be hidden for home"
    );
    assert!(
      !is_system_subcommand_hidden(&cmd, "repl"),
      "repl should be visible for home"
    );
  }
}
