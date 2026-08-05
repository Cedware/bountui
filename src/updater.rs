use anyhow::{anyhow, Context, Result};
use self_update::backends::github::ReleaseList;
use self_update::Status;

const REPO_OWNER: &str = "Cedware";
const REPO_NAME: &str = "bountui";

/// The version of this bountui binary.
///
/// Release builds are stamped with the release version via the `BOUNTUI_VERSION`
/// environment variable (set in CI), because the version in `Cargo.toml` is not
/// bumped by semantic-release. Local builds fall back to the `Cargo.toml` version.
pub fn current_version() -> &'static str {
    match option_env!("BOUNTUI_VERSION") {
        Some(version) if !version.is_empty() => version,
        _ => env!("CARGO_PKG_VERSION"),
    }
}

/// Whether this binary was stamped with a release version at build time.
///
/// Local builds report the static `Cargo.toml` version, so the automatic update
/// check on startup would prompt on every launch — it only runs for release builds.
#[cfg(not(test))]
pub fn is_release_build() -> bool {
    matches!(option_env!("BOUNTUI_VERSION"), Some(version) if !version.is_empty())
}

/// The target triple used in the GitHub release asset names
/// (`bountui-<version>-<target>.zip`).
///
/// Note: the official Linux x86_64 release is a static musl build, so even
/// binaries compiled locally on a gnu system update to the musl asset.
fn release_target() -> Result<&'static str> {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-unknown-linux-musl")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Ok("aarch64-unknown-linux-gnu")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-apple-darwin")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Ok("aarch64-apple-darwin")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-pc-windows-gnu")
    } else {
        Err(anyhow!(
            "self-update is not supported on {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    }
}

/// Fetch the version of the latest GitHub release that ships an asset for the
/// given target. Blocks on network IO.
#[cfg(not(test))]
fn fetch_latest_version(target: &str) -> Result<String> {
    let releases = ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .with_target(target)
        .build()?
        .fetch()
        .context("Failed to fetch the release list from GitHub")?;
    let latest = releases
        .first()
        .ok_or_else(|| anyhow!("No release with an asset for {target} found"))?;
    Ok(latest.version.clone())
}

/// Check whether a newer bountui release is available on GitHub.
///
/// Returns the version of the newer release, or `None` when bountui is up to
/// date. Blocks on network IO.
#[cfg(not(test))]
pub fn check_for_update() -> Result<Option<String>> {
    let latest = fetch_latest_version(release_target()?)?;
    let current = semver::Version::parse(current_version())
        .with_context(|| format!("Failed to parse current version '{}'", current_version()))?;
    let latest_version = semver::Version::parse(&latest)
        .with_context(|| format!("Failed to parse release version '{latest}'"))?;
    Ok((latest_version > current).then_some(latest))
}

/// Download the given release version from GitHub and replace the current
/// binary. Blocks while downloading; must not be called from within the tokio
/// runtime (reqwest blocking) — use `tokio::task::spawn_blocking`.
///
/// Runs silently (no stdout output) because the TUI owns the terminal.
pub fn update_to_version(version: &str) -> Result<Status> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("bountui")
        .target(release_target()?)
        .target_version_tag(&format!("v{version}"))
        .current_version(current_version())
        .show_output(false)
        .show_download_progress(false)
        .no_confirm(true)
        .build()?
        .update()?;
    Ok(status)
}
