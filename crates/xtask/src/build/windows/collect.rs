//! The bundles under stable names, as `multi-wallet-tracker-<v>-...` in the output folder.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub fn setup_name(version: &str) -> String {
    format!("multi-wallet-tracker-{version}-x64-setup.exe")
}

pub fn msi_name(version: &str, lang: &str) -> String {
    format!("multi-wallet-tracker-{version}-x64-{lang}.msi")
}

pub fn portable_name(version: &str) -> String {
    format!("multi-wallet-tracker-{version}-windows-x64-portable.zip")
}

/// `xx-XX` before `.msi` in `<product>_<version>_x64_xx-XX.msi`.
pub fn msi_lang(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".msi")?;
    let (_, lang) = stem.rsplit_once('_')?;
    let (language, region) = lang.split_once('-')?;
    let ok = language.len() == 2
        && region.len() == 2
        && language.bytes().all(|byte| byte.is_ascii_lowercase())
        && region.bytes().all(|byte| byte.is_ascii_uppercase());
    ok.then(|| lang.to_string())
}

fn files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(suffix))
        })
        .collect();
    found.sort();
    found
}

/// A file and, when the build had the updater key, its `.sig`.
fn copy_with_sig(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to).with_context(|| format!("copy {}", from.display()))?;
    let sig = PathBuf::from(format!("{}.sig", from.display()));
    if sig.is_file() {
        std::fs::copy(&sig, PathBuf::from(format!("{}.sig", to.display())))?;
    }
    Ok(())
}

/// The portable zip: the bare exe (needs the WebView2 runtime, built into Windows 10/11).
fn portable(exe: &Path, app_bin: &str, zip_path: &Path) -> Result<()> {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(zip_path)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file(format!("{app_bin}.exe"), options)?;
    writer.write_all(&std::fs::read(exe).with_context(|| format!("read {}", exe.display()))?)?;
    writer.finish()?;
    Ok(())
}

/// `release_dir` is `<target>/release` (bundles under `bundle/`, the exe next to it).
pub fn run(release_dir: &Path, out: &Path, version: &str, app_bin: &str) -> Result<()> {
    let bundle = release_dir.join("bundle");
    std::fs::create_dir_all(out)?;
    let Some(setup) = files(&bundle.join("nsis"), "-setup.exe").into_iter().next() else {
        bail!("NSIS installer not found");
    };
    copy_with_sig(&setup, &out.join(setup_name(version)))?;
    let msis = files(&bundle.join("msi"), ".msi");
    if msis.is_empty() {
        bail!("MSI not found");
    }
    for msi in msis {
        let name = msi
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let Some(lang) = msi_lang(name) else {
            bail!("no language (xx-XX) in the MSI name {name}");
        };
        copy_with_sig(&msi, &out.join(msi_name(version, &lang)))?;
    }
    portable(
        &release_dir.join(format!("{app_bin}.exe")),
        app_bin,
        &out.join(portable_name(version)),
    )
}
