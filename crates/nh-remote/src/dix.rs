use std::path::{Path, PathBuf};

use color_eyre::{
  Result,
  eyre::{Context, bail, eyre},
};
use dix::store::{StoreBackend, StorePathInfo};
use size::Size;

use super::{RemoteHost, get_flake_flags, run_remote_command};

/// A remote store path after resolving symlinks such as
/// `/run/current-system`.
#[derive(Debug, Clone)]
pub struct ResolvedRemoteStorePath {
  host: RemoteHost,
  path: PathBuf,
}

impl ResolvedRemoteStorePath {
  /// Resolve a remote path to the store path that should be queried.
  ///
  /// Direct store entries are returned unchanged. Other paths are resolved on
  /// the remote host with `readlink -f` and validated as Nix store paths.
  ///
  /// # Errors
  ///
  /// Returns an error if the remote path cannot be resolved or resolves outside
  /// `/nix/store`.
  pub fn resolve(host: &RemoteHost, path: &Path) -> Result<Self> {
    if path.parent() == Some(Path::new("/nix/store")) {
      return Ok(Self {
        host: host.clone(),
        path: path.to_path_buf(),
      });
    }

    let path = path
      .to_str()
      .ok_or_else(|| eyre!("remote path contains invalid UTF-8"))?;
    let output =
      run_remote_command(host, &["readlink", "-f", "--", path], true)?
        .ok_or_else(|| eyre!("readlink did not return a resolved path"))?;
    let mut paths = output.lines();
    let resolved_path = paths
      .next()
      .ok_or_else(|| eyre!("readlink did not return a resolved path"))?;

    if paths.next().is_some() {
      bail!("readlink returned multiple paths for one requested path");
    }

    Self::new(host, PathBuf::from(resolved_path), path)
  }

  #[must_use]
  pub fn path(&self) -> &Path {
    &self.path
  }

  /// Query this resolved remote Nix store path and convert it to dix's snapshot
  /// model.
  ///
  /// The queries run *on* the remote host over SSH (reusing the multiplexed
  /// connection) rather than through a local `--store ssh-ng://` connection.
  /// Remote ssh-ng store connections are unreliable for closure walks: Nix
  /// multiplexes its connection pool (64 by default) over a single SSH master,
  /// which trips sshd's `MaxSessions` limit; the refused sessions fall back to
  /// direct connections whose `LocalCommand=echo started` marker garbles the
  /// daemon protocol ("protocol mismatch, got 'started ...'"), and capped
  /// pools deadlock when the closure fan-out exceeds the pool. Running the
  /// query remotely lets the remote daemon walk its own store locally.
  ///
  /// # Errors
  ///
  /// Returns an error if the remote commands fail or return invalid path data.
  pub fn query_snapshot(&self) -> Result<dix::StoreSnapshot> {
    let backend = RemoteCommandBackend { host: &self.host };
    dix::query_store_snapshot_with_backend(&backend, self.path())
  }

  fn new(host: &RemoteHost, path: PathBuf, original: &str) -> Result<Self> {
    if !path.starts_with("/nix/store") {
      bail!(
        "resolved remote path '{}' for '{}' is not in /nix/store",
        path.display(),
        original
      );
    }

    Ok(Self {
      host: host.clone(),
      path,
    })
  }
}

/// A dix store backend that executes its Nix queries on the remote host via
/// SSH instead of connecting to a remote store from the local machine.
struct RemoteCommandBackend<'a> {
  host: &'a RemoteHost,
}

impl RemoteCommandBackend<'_> {
  fn run(&self, args: &[&str]) -> Result<String> {
    run_remote_command(self.host, args, true)?
      .ok_or_else(|| eyre!("remote command '{}' produced no output", args[0]))
  }
}

impl std::fmt::Display for RemoteCommandBackend<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "nix commands over ssh on '{}'", self.host)
  }
}

impl StoreBackend for RemoteCommandBackend<'_> {
  /// Does nothing (each query is its own SSH command).
  fn connect(&mut self) -> Result<()> {
    Ok(())
  }

  fn connected(&self) -> bool {
    true
  }

  fn close(&mut self) -> Result<()> {
    Ok(())
  }

  fn query_system_derivations(
    &self,
    system: &Path,
  ) -> Result<Vec<dix::StorePath>> {
    let sw = system.join("sw");
    let sw = sw
      .to_str()
      .ok_or_else(|| eyre!("remote path contains invalid UTF-8"))?;

    let output =
      self.run(&["nix-store", "--query", "--references", "--", sw])?;
    parse_store_paths(&output)
  }

  fn query_dependents(&self, path: &Path) -> Result<Vec<dix::StorePath>> {
    let path = path
      .to_str()
      .ok_or_else(|| eyre!("remote path contains invalid UTF-8"))?;

    let output =
      self.run(&["nix-store", "--query", "--requisites", "--", path])?;
    parse_store_paths(&output)
  }

  fn query_closure_path_info(&self, path: &Path) -> Result<Vec<StorePathInfo>> {
    let path = path
      .to_str()
      .ok_or_else(|| eyre!("remote path contains invalid UTF-8"))?;

    let mut args = vec!["nix", "path-info"];
    args.extend(get_flake_flags());
    args.extend(["--recursive", "--size", "--", path]);

    let output = self.run(&args)?;
    parse_path_info_sizes(&output)
  }
}

/// Parse newline-separated store paths as printed by `nix-store --query`.
fn parse_store_paths(output: &str) -> Result<Vec<dix::StorePath>> {
  output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(|line| {
      dix::StorePath::try_from(PathBuf::from(line.trim())).wrap_err_with(|| {
        format!("invalid store path in remote nix output: {line}")
      })
    })
    .collect()
}

/// Parse `<path> <nar-size>` lines as printed by `nix path-info --size`.
fn parse_path_info_sizes(output: &str) -> Result<Vec<StorePathInfo>> {
  output
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(|line| {
      let mut columns = line.split_whitespace();
      let path = columns
        .next()
        .ok_or_else(|| eyre!("missing path in nix path-info output line"))?;
      let bytes = columns
        .next()
        .ok_or_else(|| {
          eyre!("missing NAR size in nix path-info output line: {line}")
        })?
        .parse::<i64>()
        .wrap_err("failed to parse NAR size from nix path-info output")?;

      Ok(StorePathInfo::new(
        dix::StorePath::try_from(PathBuf::from(path)).wrap_err_with(|| {
          format!("invalid store path in remote nix output: {line}")
        })?,
        Size::from_bytes(bytes),
      ))
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  const BASH: &str = "/nix/store/2123456789abcdefghijklmnopqrstuv-bash-5.3";

  #[test]
  fn resolved_remote_store_path_preserves_direct_store_entry() -> Result<()> {
    let host = RemoteHost::parse("target.example")?;

    let root = ResolvedRemoteStorePath::resolve(&host, Path::new(BASH))?;

    assert_eq!(root.path(), Path::new(BASH));

    Ok(())
  }

  #[test]
  fn parse_store_paths_accepts_nix_store_query_output() -> Result<()> {
    let output = format!("{BASH}\n\n{BASH}\n");

    let paths = parse_store_paths(&output)?;

    assert_eq!(paths.len(), 2);
    Ok(())
  }

  #[test]
  fn parse_store_paths_rejects_non_store_paths() {
    assert!(parse_store_paths("/etc/passwd").is_err());
  }

  #[test]
  fn parse_path_info_sizes_accepts_two_column_output() -> Result<()> {
    let output = format!("{BASH}\t        120\n{BASH}            4096\n");

    let infos = parse_path_info_sizes(&output)?;

    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].nar_size(), Size::from_bytes(120));
    assert_eq!(infos[1].nar_size(), Size::from_bytes(4096));
    Ok(())
  }

  #[test]
  fn parse_path_info_sizes_rejects_missing_size_column() {
    assert!(parse_path_info_sizes(BASH).is_err());
  }
}
