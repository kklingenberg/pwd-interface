//! Exposes functions that build and extract bundles from paths.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::path::{Path, PathBuf};
use tar::Archive;
use tar::Builder;
use tempfile::NamedTempFile;

/// Visit files within `dir`. Source:
/// https://doc.rust-lang.org/stable/std/fs/fn.read_dir.html
fn visit_dirs<P, F>(dir: &Path, ignore: &P, cb: &mut F) -> Result<()>
where
    F: FnMut(PathBuf) -> Result<()>,
    P: Fn(&Path) -> bool,
{
    if ignore(dir) {
        // forcefully ignored
    } else if dir.is_dir() {
        for entry in dir
            .read_dir()
            .with_context(|| format!("Failed to read directory {:?}", dir))?
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            visit_dirs(&path, ignore, cb)?;
        }
    } else {
        cb(dir.to_path_buf()).with_context(|| format!("Failed to visit file {:?}", dir))?;
    }
    Ok(())
}

/// Makes a predicate to filter out files from bundles, considering a
/// posible .pwdiignore file which might exist at the base path.
fn make_ignore_predicate(base_path: &Path) -> Box<dyn Fn(&Path) -> bool> {
    let ignores_file = base_path.join(".pwdiignore");
    match gitignore::File::new(&ignores_file) {
        Ok(f) => {
            let included_files = f.included_files().unwrap_or_default();
            Box::new(move |p| !p.is_dir() && !included_files.contains(&p.to_path_buf()))
        }
        Err(_) => Box::new(|p| p.ends_with(".git")), // By default, just ignore the .git folder
    }
}

/// Holds temporary files so that they don't fade prematurely
pub struct Bundler {
    files: Vec<NamedTempFile>,
}

impl Default for Bundler {
    fn default() -> Self {
        Self::new()
    }
}

impl Bundler {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn clear(&mut self) -> Result<()> {
        while let Some(tempfile) = self.files.pop() {
            tempfile.close()?;
        }
        Ok(())
    }

    /// Makes a bundle in a temporary location and returns a path to it.
    pub fn make(&mut self, path: &Path) -> Result<PathBuf> {
        let target =
            NamedTempFile::new().context("Couldn't create temporary file to hold the bundle")?;
        let target_path = target.path().to_path_buf();
        let encoder = GzEncoder::new(target, Compression::default());
        let mut archive = Builder::new(encoder);
        let ignore = make_ignore_predicate(path);
        visit_dirs(path, &ignore, &mut |entry: PathBuf| {
            let relative = entry.as_path().strip_prefix(path).with_context(|| {
                format!("Entry {:?} is not within base path {:?}", &entry, path)
            })?;
            archive
                .append_path_with_name(entry.as_path(), relative)
                .with_context(|| format!("Couldn't add file {:?} to bundle", &entry))?;
            Ok(())
        })
        .context("Couldn't add file to bundle")?;
        let mut encoder = archive
            .into_inner()
            .context("Couldn't finish tar archive for bundle")?;
        encoder
            .try_finish()
            .context("Couldn't finish gzip file for bundle")?;
        self.files.push(encoder.finish()?);
        Ok(target_path)
    }

    /// Extracts the given bundle into the given target.
    pub fn extract(&self, bundle_path: &Path, target_path: &Path) -> Result<()> {
        let bundle_file = File::open(bundle_path)
            .with_context(|| format!("Couldn't open file {:?}", bundle_path))?;
        let decoder = GzDecoder::new(bundle_file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(target_path)
            .with_context(|| format!("Couldn't unpack archive {:?}", bundle_path))?;
        Ok(())
    }
}
