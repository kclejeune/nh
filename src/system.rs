//! Implementation of the `nh system` command that delegates to os/darwin/home
//! based on the `NH_SYSTEM_TYPE` environment variable.

use color_eyre::eyre::{Result, bail};
use tracing::debug;

use crate::{
  checks::{FeatureRequirements, FlakeFeatures},
  commands::ElevationStrategy,
  interface::{
    DarwinArgs,
    DarwinRebuildArgs,
    DarwinReplArgs,
    DarwinSubcommand,
    HomeArgs,
    HomeRebuildArgs,
    HomeReplArgs,
    HomeSubcommand,
    OsArgs,
    OsRebuildActivateArgs,
    OsRebuildArgs,
    OsReplArgs,
    OsSubcommand,
    SystemArgs,
    SystemRebuildArgs,
    SystemReplArgs,
    SystemSubcommand,
    SystemType,
  },
};

impl SystemArgs {
  /// Run the system command by delegating to the appropriate target
  pub fn run(self, elevation: ElevationStrategy) -> Result<()> {
    let system_type =
      SystemType::from_env().map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

    debug!("NH_SYSTEM_TYPE resolved to: {:?}", system_type);

    // Set NH_CURRENT_COMMAND based on the target system type
    let command_name = match system_type {
      SystemType::Os => "os",
      SystemType::Darwin => "darwin",
      SystemType::Home => "home",
    };
    unsafe {
      std::env::set_var("NH_CURRENT_COMMAND", command_name);
    }

    match self.subcommand {
      SystemSubcommand::Switch(args) => {
        run_switch(system_type, args, elevation)
      },
      SystemSubcommand::Build(args) => run_build(system_type, args, elevation),
      SystemSubcommand::Boot(args) => run_boot(system_type, args, elevation),
      SystemSubcommand::Test(args) => run_test(system_type, args, elevation),
      SystemSubcommand::Repl(args) => run_repl(system_type, args),
    }
  }

  /// Get feature requirements for the system command
  /// Since we don't know the target until runtime, we assume flake features are
  /// needed
  #[must_use]
  pub fn get_feature_requirements(&self) -> Box<dyn FeatureRequirements> {
    Box::new(FlakeFeatures)
  }
}

fn run_switch(
  system_type: SystemType,
  args: SystemRebuildArgs,
  elevation: ElevationStrategy,
) -> Result<()> {
  match system_type {
    SystemType::Os => {
      let os_args = args.to_os_rebuild_activate_args();
      OsArgs {
        subcommand: OsSubcommand::Switch(os_args),
      }
      .run(elevation)
    },
    SystemType::Darwin => {
      let darwin_args = args.to_darwin_rebuild_args();
      DarwinArgs {
        subcommand: DarwinSubcommand::Switch(darwin_args),
      }
      .run(elevation)
    },
    SystemType::Home => {
      let home_args = args.to_home_rebuild_args();
      HomeArgs {
        subcommand: HomeSubcommand::Switch(home_args),
      }
      .run()
    },
  }
}

fn run_build(
  system_type: SystemType,
  args: SystemRebuildArgs,
  elevation: ElevationStrategy,
) -> Result<()> {
  match system_type {
    SystemType::Os => {
      let os_args = args.to_os_rebuild_args();
      OsArgs {
        subcommand: OsSubcommand::Build(os_args),
      }
      .run(elevation)
    },
    SystemType::Darwin => {
      let darwin_args = args.to_darwin_rebuild_args();
      DarwinArgs {
        subcommand: DarwinSubcommand::Build(darwin_args),
      }
      .run(elevation)
    },
    SystemType::Home => {
      let home_args = args.to_home_rebuild_args();
      HomeArgs {
        subcommand: HomeSubcommand::Build(home_args),
      }
      .run()
    },
  }
}

fn run_boot(
  system_type: SystemType,
  args: SystemRebuildArgs,
  elevation: ElevationStrategy,
) -> Result<()> {
  match system_type {
    SystemType::Os => {
      let os_args = args.to_os_rebuild_activate_args();
      OsArgs {
        subcommand: OsSubcommand::Boot(os_args),
      }
      .run(elevation)
    },
    SystemType::Darwin | SystemType::Home => {
      bail!(
        "The 'boot' subcommand is only supported for NixOS (os).\nCurrent \
         NH_SYSTEM_TYPE: {:?}\nAvailable subcommands for {:?}: switch, build, \
         repl",
        system_type,
        system_type
      )
    },
  }
}

fn run_test(
  system_type: SystemType,
  args: SystemRebuildArgs,
  elevation: ElevationStrategy,
) -> Result<()> {
  match system_type {
    SystemType::Os => {
      let os_args = args.to_os_rebuild_activate_args();
      OsArgs {
        subcommand: OsSubcommand::Test(os_args),
      }
      .run(elevation)
    },
    SystemType::Darwin | SystemType::Home => {
      bail!(
        "The 'test' subcommand is only supported for NixOS (os).\nCurrent \
         NH_SYSTEM_TYPE: {:?}\nAvailable subcommands for {:?}: switch, build, \
         repl",
        system_type,
        system_type
      )
    },
  }
}

fn run_repl(system_type: SystemType, args: SystemReplArgs) -> Result<()> {
  match system_type {
    SystemType::Os => {
      let os_args = args.to_os_repl_args();
      OsArgs {
        subcommand: OsSubcommand::Repl(os_args),
      }
      .run(ElevationStrategy::Auto)
    },
    SystemType::Darwin => {
      let darwin_args = args.to_darwin_repl_args();
      DarwinArgs {
        subcommand: DarwinSubcommand::Repl(darwin_args),
      }
      .run(ElevationStrategy::Auto)
    },
    SystemType::Home => {
      let home_args = args.to_home_repl_args();
      HomeArgs {
        subcommand: HomeSubcommand::Repl(home_args),
      }
      .run()
    },
  }
}

// Conversion implementations for SystemRebuildArgs

impl SystemRebuildArgs {
  /// Convert to `OsRebuildArgs` for the `build` subcommand
  #[must_use]
  pub fn to_os_rebuild_args(&self) -> OsRebuildArgs {
    OsRebuildArgs {
      common:             self.common.clone(),
      update_args:        self.update_args.clone(),
      hostname:           self.hostname.clone(),
      specialisation:     self.specialisation.clone(),
      no_specialisation:  self.no_specialisation,
      install_bootloader: self.install_bootloader,
      extra_args:         self.extra_args.clone(),
      bypass_root_check:  self.bypass_root_check,
      target_host:        self.target_host.clone(),
      build_host:         self.build_host.clone(),
    }
  }

  /// Convert to `OsRebuildActivateArgs` for switch/boot/test subcommands
  #[must_use]
  pub fn to_os_rebuild_activate_args(&self) -> OsRebuildActivateArgs {
    OsRebuildActivateArgs {
      rebuild:              self.to_os_rebuild_args(),
      show_activation_logs: self.show_activation_logs,
    }
  }

  /// Convert to `DarwinRebuildArgs`
  #[must_use]
  pub fn to_darwin_rebuild_args(&self) -> DarwinRebuildArgs {
    DarwinRebuildArgs {
      common:               self.common.clone(),
      update_args:          self.update_args.clone(),
      hostname:             self.hostname.clone(),
      extra_args:           self.extra_args.clone(),
      bypass_root_check:    self.bypass_root_check,
      show_activation_logs: self.show_activation_logs,
    }
  }

  /// Convert to `HomeRebuildArgs`
  #[must_use]
  pub fn to_home_rebuild_args(&self) -> HomeRebuildArgs {
    HomeRebuildArgs {
      common:               self.common.clone(),
      update_args:          self.update_args.clone(),
      // For home, prefer --configuration over --hostname
      configuration:        self
        .configuration
        .clone()
        .or_else(|| self.hostname.clone()),
      specialisation:       self.specialisation.clone(),
      no_specialisation:    self.no_specialisation,
      extra_args:           self.extra_args.clone(),
      backup_extension:     self.backup_extension.clone(),
      show_activation_logs: self.show_activation_logs,
    }
  }
}

// Conversion implementations for SystemReplArgs

impl SystemReplArgs {
  /// Convert to `OsReplArgs`
  #[must_use]
  pub fn to_os_repl_args(&self) -> OsReplArgs {
    OsReplArgs {
      installable: self.installable.clone(),
      hostname:    self.hostname.clone(),
    }
  }

  /// Convert to `DarwinReplArgs`
  #[must_use]
  pub fn to_darwin_repl_args(&self) -> DarwinReplArgs {
    DarwinReplArgs {
      installable: self.installable.clone(),
      hostname:    self.hostname.clone(),
    }
  }

  /// Convert to `HomeReplArgs`
  #[must_use]
  pub fn to_home_repl_args(&self) -> HomeReplArgs {
    HomeReplArgs {
      installable:   self.installable.clone(),
      // For home, prefer --configuration over --hostname
      configuration: self
        .configuration
        .clone()
        .or_else(|| self.hostname.clone()),
      extra_args:    self.extra_args.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::env;

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

  #[test]
  #[serial]
  fn test_system_type_from_env_os() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "os");
    let result = SystemType::from_env();
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), SystemType::Os));
  }

  #[test]
  #[serial]
  fn test_system_type_from_env_darwin() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "darwin");
    let result = SystemType::from_env();
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), SystemType::Darwin));
  }

  #[test]
  #[serial]
  fn test_system_type_from_env_home() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "home");
    let result = SystemType::from_env();
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), SystemType::Home));
  }

  #[test]
  #[serial]
  fn test_system_type_from_env_invalid() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "invalid");
    let result = SystemType::from_env();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Invalid NH_SYSTEM_TYPE value"));
    assert!(err.contains("invalid"));
  }

  #[test]
  #[serial]
  fn test_system_type_from_env_not_set() {
    let _guard = EnvGuard::remove("NH_SYSTEM_TYPE");
    let result = SystemType::from_env();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("NH_SYSTEM_TYPE environment variable is not set"));
  }

  #[test]
  #[serial]
  fn test_system_type_from_env_empty() {
    let _guard = EnvGuard::new("NH_SYSTEM_TYPE", "");
    let result = SystemType::from_env();
    assert!(result.is_err());
    // Empty string is treated as invalid value
    let err = result.unwrap_err();
    assert!(err.contains("Invalid NH_SYSTEM_TYPE value"));
  }

  #[test]
  fn test_home_rebuild_args_configuration_takes_precedence() {
    // When both configuration and hostname are set, configuration should be
    // used
    use crate::{
      installable::Installable,
      interface::{
        CommonRebuildArgs,
        DiffType,
        NixBuildPassthroughArgs,
        UpdateArgs,
      },
    };

    let args = SystemRebuildArgs {
      common:               CommonRebuildArgs {
        dry:         false,
        ask:         false,
        installable: Installable::Unspecified,
        no_nom:      false,
        out_link:    None,
        diff:        DiffType::Auto,
        passthrough: NixBuildPassthroughArgs::default(),
      },
      update_args:          UpdateArgs {
        update_all:   false,
        update_input: None,
      },
      hostname:             Some("my-hostname".to_string()),
      configuration:        Some("my-config".to_string()),
      specialisation:       None,
      no_specialisation:    false,
      install_bootloader:   false,
      extra_args:           vec![],
      bypass_root_check:    false,
      target_host:          None,
      build_host:           None,
      backup_extension:     None,
      show_activation_logs: false,
    };

    let home_args = args.to_home_rebuild_args();
    // configuration should take precedence
    assert_eq!(home_args.configuration, Some("my-config".to_string()));
  }

  #[test]
  fn test_home_rebuild_args_hostname_fallback() {
    // When only hostname is set, it should be used as configuration
    use crate::{
      installable::Installable,
      interface::{
        CommonRebuildArgs,
        DiffType,
        NixBuildPassthroughArgs,
        UpdateArgs,
      },
    };

    let args = SystemRebuildArgs {
      common:               CommonRebuildArgs {
        dry:         false,
        ask:         false,
        installable: Installable::Unspecified,
        no_nom:      false,
        out_link:    None,
        diff:        DiffType::Auto,
        passthrough: NixBuildPassthroughArgs::default(),
      },
      update_args:          UpdateArgs {
        update_all:   false,
        update_input: None,
      },
      hostname:             Some("my-hostname".to_string()),
      configuration:        None,
      specialisation:       None,
      no_specialisation:    false,
      install_bootloader:   false,
      extra_args:           vec![],
      bypass_root_check:    false,
      target_host:          None,
      build_host:           None,
      backup_extension:     None,
      show_activation_logs: false,
    };

    let home_args = args.to_home_rebuild_args();
    // hostname should be used as fallback
    assert_eq!(home_args.configuration, Some("my-hostname".to_string()));
  }

  #[test]
  fn test_home_repl_args_hostname_fallback() {
    use crate::installable::Installable;

    let args = SystemReplArgs {
      installable:   Installable::Unspecified,
      hostname:      Some("my-hostname".to_string()),
      configuration: None,
      extra_args:    vec![],
    };

    let home_args = args.to_home_repl_args();
    assert_eq!(home_args.configuration, Some("my-hostname".to_string()));
  }

  #[test]
  fn test_os_rebuild_args_preserves_fields() {
    use crate::{
      installable::Installable,
      interface::{
        CommonRebuildArgs,
        DiffType,
        NixBuildPassthroughArgs,
        UpdateArgs,
      },
    };

    let args = SystemRebuildArgs {
      common:               CommonRebuildArgs {
        dry:         true,
        ask:         true,
        installable: Installable::Unspecified,
        no_nom:      true,
        out_link:    None,
        diff:        DiffType::Always,
        passthrough: NixBuildPassthroughArgs::default(),
      },
      update_args:          UpdateArgs {
        update_all:   true,
        update_input: None,
      },
      hostname:             Some("my-host".to_string()),
      configuration:        None,
      specialisation:       Some("gaming".to_string()),
      no_specialisation:    false,
      install_bootloader:   true,
      extra_args:           vec!["--verbose".to_string()],
      bypass_root_check:    true,
      target_host:          Some("remote".to_string()),
      build_host:           Some("builder".to_string()),
      backup_extension:     None,
      show_activation_logs: true,
    };

    let os_args = args.to_os_rebuild_args();
    assert!(os_args.common.dry);
    assert!(os_args.common.ask);
    assert!(os_args.common.no_nom);
    assert!(os_args.update_args.update_all);
    assert_eq!(os_args.hostname, Some("my-host".to_string()));
    assert_eq!(os_args.specialisation, Some("gaming".to_string()));
    assert!(os_args.install_bootloader);
    assert_eq!(os_args.extra_args, vec!["--verbose".to_string()]);
    assert!(os_args.bypass_root_check);
    assert_eq!(os_args.target_host, Some("remote".to_string()));
    assert_eq!(os_args.build_host, Some("builder".to_string()));
  }

  #[test]
  fn test_darwin_rebuild_args_preserves_fields() {
    use crate::{
      installable::Installable,
      interface::{
        CommonRebuildArgs,
        DiffType,
        NixBuildPassthroughArgs,
        UpdateArgs,
      },
    };

    let args = SystemRebuildArgs {
      common:               CommonRebuildArgs {
        dry:         true,
        ask:         false,
        installable: Installable::Unspecified,
        no_nom:      false,
        out_link:    None,
        diff:        DiffType::Never,
        passthrough: NixBuildPassthroughArgs::default(),
      },
      update_args:          UpdateArgs {
        update_all:   false,
        update_input: Some(vec!["nixpkgs".to_string()]),
      },
      hostname:             Some("macbook".to_string()),
      configuration:        None,
      specialisation:       None,
      no_specialisation:    false,
      install_bootloader:   false,
      extra_args:           vec![],
      bypass_root_check:    true,
      target_host:          None,
      build_host:           None,
      backup_extension:     None,
      show_activation_logs: true,
    };

    let darwin_args = args.to_darwin_rebuild_args();
    assert!(darwin_args.common.dry);
    assert!(!darwin_args.common.ask);
    assert_eq!(
      darwin_args.update_args.update_input,
      Some(vec!["nixpkgs".to_string()])
    );
    assert_eq!(darwin_args.hostname, Some("macbook".to_string()));
    assert!(darwin_args.bypass_root_check);
    assert!(darwin_args.show_activation_logs);
  }
}
