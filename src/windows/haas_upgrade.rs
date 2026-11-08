//! `hex haas-upgrade` (Haas fork only): update this portable install from
//! the fork's GitHub releases.
//!
//! Each Haas package carries a `haas-build.json` marker next to `hex.exe`
//! (variant, channel, version, build). Each published release carries a
//! `manifest.json` with the same fields plus every variant's asset name,
//! size and SHA-256. The command compares the two, downloads its own
//! variant's zip, verifies size and SHA-256, and swaps the files in place.
//! Windows allows renaming a running executable, so `hex.exe` itself is
//! renamed aside (`hex.exe.upgrade-old`) and replaced; the other files are
//! lock-checked first, so a running Hex makes the command stop before it
//! changes anything instead of leaving a half-swapped folder.
//!
//! Channels: `stable` follows the newest `hex-v*` release, `daily`
//! follows the rolling `haas-daily` prerelease. Packages published before
//! the marker existed have no `haas-build.json`; one run with `--variant`
//! installs a current package and writes the marker.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "ymkn/hex-haas";
const MARKER_FILE: &str = "haas-build.json";
const OLD_EXE_FILE: &str = "hex.exe.upgrade-old";
const MANIFEST_LIMIT: u64 = 256 * 1024;
const DOWNLOAD_LIMIT: u64 = 1024 * 1024 * 1024;

/// Variants this binary's architecture can install.
const VARIANTS: &[&str] = if cfg!(target_arch = "aarch64") {
    &["arm64"]
} else {
    &["gpu", "cpu"]
};

#[derive(Deserialize)]
struct BuildMarker {
    variant: String,
    channel: String,
    version: String,
    build: String,
}

#[derive(Deserialize)]
struct Manifest {
    channel: String,
    version: String,
    build: String,
    assets: BTreeMap<String, Asset>,
}

#[derive(Deserialize)]
struct Asset {
    file: String,
    sha256: String,
    size: u64,
}

pub fn run(check_only: bool, yes: bool, channel: Option<String>, variant: Option<String>) -> u8 {
    match execute(check_only, yes, channel, variant) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("haas-upgrade: {error:#}");
            1
        }
    }
}

fn execute(
    check_only: bool,
    yes: bool,
    channel_arg: Option<String>,
    variant_arg: Option<String>,
) -> Result<()> {
    let exe = std::env::current_exe().wrap_err("cannot locate the running hex.exe")?;
    let dir = exe
        .parent()
        .ok_or_else(|| eyre!("hex.exe has no parent folder"))?
        .to_path_buf();

    // A previous upgrade's renamed-aside executable is dead weight once its
    // process is gone; remove it best-effort (it stays if still locked).
    let stale = dir.join(OLD_EXE_FILE);
    if stale.is_file() && stale != exe {
        let _ = fs::remove_file(&stale);
    }

    let mut local = read_marker(&dir, variant_arg.as_deref())?;
    if let Some(variant) = &variant_arg {
        local.variant = variant.clone();
    }
    if !VARIANTS.contains(&local.variant.as_str()) {
        bail!(
            "variant '{}' cannot run on this architecture (expected one of: {})",
            local.variant,
            VARIANTS.join(", ")
        );
    }
    let channel = channel_arg.unwrap_or_else(|| local.channel.clone());
    if channel != "stable" && channel != "daily" {
        bail!("channel must be 'stable' or 'daily', not '{channel}'");
    }

    println!(
        "Installed: Hex Haas {} ({} channel, {} build)",
        local.version, local.variant, local.build
    );
    let manifest = fetch_manifest(&channel, &local.version)?;
    if manifest.channel != channel {
        bail!(
            "the {channel} channel served a '{}' manifest; refusing to use it",
            manifest.channel
        );
    }
    match version_parts(&manifest.version).cmp(&version_parts(&local.version)) {
        Ordering::Less => {
            println!(
                "The {channel} channel is older ({}); nothing to do.",
                manifest.version
            );
            return Ok(());
        }
        Ordering::Equal if manifest.build <= local.build => {
            println!("Already up to date on the {channel} channel.");
            return Ok(());
        }
        _ => {}
    }
    let asset = manifest.assets.get(&local.variant).ok_or_else(|| {
        eyre!(
            "the {channel} manifest has no asset for variant '{}'",
            local.variant
        )
    })?;
    println!(
        "Update available: {} build {} -> {} build {} ({}, {:.1} MiB)",
        local.version,
        local.build,
        manifest.version,
        manifest.build,
        asset.file,
        asset.size as f64 / 1_048_576.0
    );
    if check_only {
        println!("Run `hex haas-upgrade` to install it.");
        return Ok(());
    }
    if !yes {
        print!("Download and install? [y/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Aborted; nothing was changed.");
            return Ok(());
        }
    }

    let scratch = std::env::temp_dir().join(format!("hex-haas-upgrade-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).wrap_err("cannot create the upgrade scratch folder")?;
    let outcome = download_verify_and_swap(&channel, &local.version, asset, &dir, &exe, &scratch);
    let _ = fs::remove_dir_all(&scratch);
    outcome?;
    println!(
        "Updated to {} build {}. Restart Hex to run it.",
        manifest.version, manifest.build
    );
    println!("The previous hex.exe is kept as {OLD_EXE_FILE} until a later run removes it.");
    Ok(())
}

/// The local build marker, or a synthesized one for pre-marker packages
/// when the user names the variant once with `--variant`.
fn read_marker(dir: &Path, variant_arg: Option<&str>) -> Result<BuildMarker> {
    let path = dir.join(MARKER_FILE);
    if path.is_file() {
        let text = fs::read_to_string(&path).wrap_err("cannot read haas-build.json")?;
        let marker: BuildMarker =
            serde_json::from_str(&text).wrap_err("haas-build.json is not valid")?;
        return Ok(marker);
    }
    match variant_arg {
        Some(variant) => Ok(BuildMarker {
            variant: variant.to_owned(),
            channel: "stable".to_owned(),
            version: crate::VERSION.to_owned(),
            build: String::new(),
        }),
        None => bail!(
            "no {MARKER_FILE} next to hex.exe: this package predates update markers. \
             Run `hex haas-upgrade --variant {}` once, with the variant of this package ({}), \
             to move onto a marked package.",
            VARIANTS[0],
            VARIANTS.join(" or ")
        ),
    }
}

fn manifest_url(channel: &str) -> String {
    if channel == "daily" {
        format!("https://github.com/{REPO}/releases/download/haas-daily/manifest.json")
    } else {
        format!("https://github.com/{REPO}/releases/latest/download/manifest.json")
    }
}

fn asset_url(channel: &str, file: &str) -> String {
    if channel == "daily" {
        format!("https://github.com/{REPO}/releases/download/haas-daily/{file}")
    } else {
        format!("https://github.com/{REPO}/releases/latest/download/{file}")
    }
}

fn fetch_manifest(channel: &str, version: &str) -> Result<Manifest> {
    let url = manifest_url(channel);
    let agent = crate::net::agent(&url, |config| {
        config
            .https_only(true)
            .max_redirects(10)
            .http_status_as_error(true)
            .timeout_global(Some(Duration::from_secs(60)))
    });
    let mut response = agent
        .get(&url)
        .header("User-Agent", format!("hex-haas/{channel}/{version}"))
        .call()
        .map_err(|error| {
            eyre!(
                "{}. If this is a 404, the newest {channel} release predates manifest.json; \
                 install one current package by hand and upgrades work from then on.",
                crate::net::unreachable(&url, &error)
            )
        })?;
    let mut text = String::new();
    response
        .body_mut()
        .with_config()
        .limit(MANIFEST_LIMIT + 1)
        .reader()
        .take(MANIFEST_LIMIT + 1)
        .read_to_string(&mut text)
        .wrap_err("the release manifest is not UTF-8")?;
    if text.len() as u64 > MANIFEST_LIMIT {
        bail!("the release manifest is too large");
    }
    serde_json::from_str(&text).wrap_err("the release manifest is not valid JSON")
}

/// Numeric components of a version, for ordering: "3.3.1" -> [3, 3, 1].
/// Non-numeric components (a `-dev` tag) drop out, and builds order by
/// their `yyyyMMdd-HHmm` string, so versions compare first, builds second.
fn version_parts(version: &str) -> Vec<u64> {
    version
        .split(['.', '-'])
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn download_verify_and_swap(
    channel: &str,
    version: &str,
    asset: &Asset,
    dir: &Path,
    exe: &Path,
    scratch: &Path,
) -> Result<()> {
    let url = asset_url(channel, &asset.file);
    let zip_path = scratch.join(&asset.file);
    println!("Downloading {} ...", asset.file);
    let agent = crate::net::agent(&url, |config| {
        config
            .https_only(true)
            .max_redirects(10)
            .http_status_as_error(true)
            .timeout_global(Some(Duration::from_secs(15 * 60)))
    });
    let mut response = agent
        .get(&url)
        .header("User-Agent", format!("hex-haas/{channel}/{version}"))
        .call()
        .map_err(|error| eyre!("{}", crate::net::unreachable(&url, &error)))?;
    let mut out = File::create(&zip_path).wrap_err("cannot create the downloaded zip")?;
    let mut reader = response.body_mut().with_config().limit(DOWNLOAD_LIMIT).reader();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let read = reader.read(&mut buffer).wrap_err("download failed midway")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])
            .wrap_err("cannot write the downloaded zip")?;
        total += read as u64;
    }
    drop(out);
    if total != asset.size {
        bail!(
            "downloaded size {total} does not match the manifest's {}",
            asset.size
        );
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if digest != asset.sha256 {
        bail!("downloaded SHA-256 {digest} does not match the manifest's {}", asset.sha256);
    }
    println!("Verified against the manifest (size and SHA-256).");

    let staging = scratch.join("staging");
    fs::create_dir_all(&staging).wrap_err("cannot create the staging folder")?;
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(&staging)
        .status()
        .wrap_err("cannot run tar.exe to unpack the update")?;
    if !status.success() {
        bail!("tar.exe failed to unpack the update (exit {status})");
    }
    let root = content_root(&staging)?;
    swap_in(&root, dir, exe)
}

/// The folder holding the package files: the staging folder itself, or its
/// single child folder when the zip wraps one.
fn content_root(staging: &Path) -> Result<PathBuf> {
    let entries: Vec<PathBuf> = fs::read_dir(staging)
        .wrap_err("cannot read the unpacked update")?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    if entries.len() == 1 && entries[0].is_dir() {
        return Ok(entries[0].clone());
    }
    Ok(staging.to_path_buf())
}

fn swap_in(root: &Path, dir: &Path, exe: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(root).wrap_err("cannot read the unpacked update")? {
        let path = entry?.path();
        if path.is_file() {
            files.push(path);
        }
    }
    if !files.iter().any(|path| path.file_name() == exe.file_name()) {
        bail!("the update package does not contain hex.exe");
    }
    // Lock preflight: every existing target but the running executable
    // itself must be writable, so a running Hex (its speech worker locks
    // speech.exe/speech.dll) stops the upgrade before anything changes.
    for path in &files {
        let name = path.file_name().unwrap();
        if Some(name) == exe.file_name() {
            continue;
        }
        let target = dir.join(name);
        if target.is_file() {
            OpenOptions::new().write(true).open(&target).map_err(|error| {
                eyre!(
                    "{} is in use ({error}). Quit Hex completely, then run `hex haas-upgrade` again.",
                    target.display()
                )
            })?;
        }
    }
    // Install plain files first, then hex.exe, then the marker last: the
    // marker only ever describes a fully swapped install.
    let exe_name = exe.file_name().unwrap();
    let marker_name = std::ffi::OsStr::new(MARKER_FILE);
    let mut ordered: Vec<&PathBuf> = files
        .iter()
        .filter(|path| {
            let name = path.file_name().unwrap();
            name != exe_name && name != marker_name
        })
        .collect();
    if let Some(path) = files.iter().find(|path| path.file_name().unwrap() == exe_name) {
        ordered.push(path);
    }
    if let Some(path) = files
        .iter()
        .find(|path| path.file_name().unwrap() == marker_name)
    {
        ordered.push(path);
    }
    let backup = dir.join(OLD_EXE_FILE);
    let mut renamed = false;
    for path in ordered {
        let name = path.file_name().unwrap();
        let target = dir.join(name);
        if name == exe_name {
            if backup.is_file() {
                let _ = fs::remove_file(&backup);
            }
            fs::rename(exe, &backup).wrap_err("cannot rename the running hex.exe aside")?;
            renamed = true;
        }
        if let Err(error) = fs::copy(path, &target) {
            if renamed {
                // Roll the executable back so the folder still runs.
                let _ = fs::remove_file(exe);
                let _ = fs::rename(&backup, exe);
            }
            return Err(error).wrap_err_with(|| format!("cannot install {}", target.display()));
        }
    }
    Ok(())
}
