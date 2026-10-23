//! Local resource tree: all data/assets from the official JAR, plus the complete
//! Mojang asset index (sounds, languages, fonts, panoramas, etc.). No Java classes.
use super::{checked, download};
use anyhow::{ensure, Context, Result};
use rayon::prelude::*;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    sync::atomic::{AtomicUsize, Ordering},
};

pub fn extract(zip: &mut zip::ZipArchive<fs::File>, version_root: &Path) -> Result<()> {
    let resources = version_root.join("resources");
    let mut files = 0usize;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir()
            || !(entry.name().starts_with("assets/") || entry.name().starts_with("data/"))
        {
            continue;
        }
        let path = entry
            .enclosed_name()
            .context("Unsafe resource archive path")?;
        ensure!(
            entry.size() <= 128 * 1024 * 1024,
            "Resource too large: {}",
            entry.name()
        );
        total = total
            .checked_add(entry.size())
            .context("Resource size overflow")?;
        ensure!(
            total <= 1024 * 1024 * 1024,
            "Resource archive exceeds one GiB"
        );
        let target = resources.join(path);
        fs::create_dir_all(target.parent().unwrap())?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        if !target.exists() || fs::read(&target)? != bytes {
            fs::write(target, bytes)?;
        }
        files += 1;
    }
    fs::write(resources.join("jar-resource-count.txt"), files.to_string())?;
    println!(
        "Imported {files} official resource/data files ({:.1} MiB)",
        total as f64 / 1048576.
    );
    Ok(())
}
#[derive(Clone, Deserialize)]
struct Object {
    hash: String,
    size: u64,
}
fn validate_hash(hash: &str) -> Result<()> {
    ensure!(
        hash.len() == 40 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid asset hash"
    );
    Ok(())
}
fn relative(name: &str) -> Result<&Path> {
    let path = Path::new(name);
    ensure!(
        !name.contains('\\') && path.components().all(|c| matches!(c, Component::Normal(_))),
        "Unsafe asset path: {name}"
    );
    Ok(path)
}
pub fn sync_all(
    client: &reqwest::blocking::Client,
    index: &serde_json::Value,
    version_root: &Path,
) -> Result<()> {
    let objects: BTreeMap<String, Object> = serde_json::from_value(index["objects"].clone())?;
    let cache = version_root
        .parent()
        .context("No asset cache")?
        .join("objects");
    let mut unique = BTreeMap::new();
    for (name, obj) in &objects {
        relative(name)?;
        validate_hash(&obj.hash)?;
        ensure!(
            obj.size <= 128 * 1024 * 1024,
            "Asset exceeds maximum size: {name}"
        );
        unique.insert(obj.hash.clone(), obj.clone());
    }
    let total = unique.len();
    let done = AtomicUsize::new(0);
    println!(
        "Synchronizing {total} official asset objects ({:.1} MiB)",
        unique.values().map(|o| o.size).sum::<u64>() as f64 / 1048576.
    );
    let pool = rayon::ThreadPoolBuilder::new().num_threads(6).build()?;
    pool.install(|| {
        unique.par_iter().try_for_each(|(hash, obj)| -> Result<()> {
            let target = cache.join(&hash[..2]).join(hash);
            let valid = target.exists()
                && fs::metadata(&target)?.len() == obj.size
                && checked(&fs::read(&target)?, hash).is_ok();
            if !valid {
                let url = format!(
                    "https://resources.download.minecraft.net/{}/{hash}",
                    &hash[..2]
                );
                let mut result = Err(anyhow::anyhow!("Asset download not attempted"));
                for _ in 0..3 {
                    result = download(client, &url, obj.size).and_then(|bytes| {
                        ensure!(bytes.len() as u64 == obj.size, "Incomplete asset {hash}");
                        checked(&bytes, hash)?;
                        Ok(bytes)
                    });
                    if result.is_ok() {
                        break;
                    }
                }
                let bytes = result.with_context(|| format!("Downloading asset {hash}"))?;
                fs::create_dir_all(target.parent().unwrap())?;
                let temporary = target.with_extension("part");
                fs::write(&temporary, bytes)?;
                fs::rename(temporary, &target)?;
            }
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_multiple_of(128) || n == total {
                println!("Official assets: {n}/{total}");
            }
            Ok(())
        })
    })?;
    let resource_root = version_root.join("resources/assets");
    for (name, obj) in objects {
        let target = resource_root.join(relative(&name)?);
        let source = cache.join(&obj.hash[..2]).join(&obj.hash);
        fs::create_dir_all(target.parent().unwrap())?;
        if target.exists() && checked(&fs::read(&target)?, &obj.hash).is_ok() {
            continue;
        }
        let temp = target.with_extension("optimc-part");
        if temp.exists() {
            fs::remove_file(&temp)?;
        }
        if fs::hard_link(&source, &temp).is_err() {
            fs::copy(source, &temp)?;
        }
        fs::rename(temp, target)?;
    }
    fs::write(
        version_root.join("asset-index.json"),
        serde_json::to_vec(index)?,
    )?;
    println!(
        "Full official asset tree is ready in {}",
        resource_root.display()
    );
    Ok(())
}
pub fn resource(version_root: &Path, name: &str) -> Result<std::path::PathBuf> {
    Ok(version_root
        .join("resources/assets/minecraft")
        .join(relative(name)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_cannot_escape_cache() {
        for invalid in ["../file", "/root/file", "x/../../y", "..", "x\\y"] {
            assert!(relative(invalid).is_err());
        }
        assert!(relative("textures/item/apple.png").is_ok());
    }
}
