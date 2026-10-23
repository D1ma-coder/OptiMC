pub mod builtins;
pub mod catalog;
pub mod collision;
pub mod entity_models;
pub mod held_items;
pub mod humanoid_attack;
pub mod icons;
pub mod item_icons;
pub mod mipmaps;
pub mod models;
pub mod signs;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub const VERSION: &str = "26.2";
pub const PROTOCOL: i32 = 776;
const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const TEXTURES: [&str; 6] = [
    "grass_block_top",
    "grass_block_side",
    "dirt",
    "stone",
    "sand",
    "oak_log",
];
pub const GUI_FILES: [(&str, &str); 15] = [
    (
        "textures/gui/title/background/panorama_0.png",
        "panorama_0.png",
    ),
    (
        "textures/gui/title/background/panorama_1.png",
        "panorama_1.png",
    ),
    (
        "textures/gui/title/background/panorama_2.png",
        "panorama_2.png",
    ),
    (
        "textures/gui/title/background/panorama_3.png",
        "panorama_3.png",
    ),
    (
        "textures/gui/title/background/panorama_4.png",
        "panorama_4.png",
    ),
    (
        "textures/gui/title/background/panorama_5.png",
        "panorama_5.png",
    ),
    ("textures/gui/title/minecraft.png", "title.png"),
    ("textures/gui/title/edition.png", "edition.png"),
    ("textures/gui/sprites/widget/button.png", "button.png"),
    (
        "textures/gui/sprites/widget/button_highlighted.png",
        "button_hover.png",
    ),
    (
        "textures/gui/sprites/widget/button_disabled.png",
        "button_disabled.png",
    ),
    ("textures/font/ascii.png", "ascii.png"),
    (
        "textures/font/nonlatin_european.png",
        "nonlatin_european.png",
    ),
    ("textures/font/accented.png", "accented.png"),
    ("font/include/default.json", "font.json"),
];
#[derive(Deserialize)]
struct Manifest {
    versions: Vec<Version>,
}
#[derive(Deserialize)]
struct Version {
    id: String,
    url: String,
    sha1: String,
}
#[derive(Deserialize)]
struct Metadata {
    downloads: Downloads,
    #[serde(rename = "assetIndex")]
    asset_index: Download,
}
#[derive(Deserialize)]
struct Downloads {
    client: Download,
}
#[derive(Deserialize)]
struct Download {
    url: String,
    sha1: String,
    size: u64,
}

fn checked(bytes: &[u8], hash: &str) -> Result<()> {
    let actual = format!("{:x}", Sha1::digest(bytes));
    if actual != hash {
        bail!("SHA-1 mismatch: expected {hash}, got {actual}")
    }
    Ok(())
}
fn trusted_url(url: &str) -> Result<()> {
    let url = reqwest::Url::parse(url)?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some(
                "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launcher.mojang.com"
                    | "resources.download.minecraft.net"
            )
        )
    {
        bail!("Untrusted asset origin: {url}")
    }
    Ok(())
}
fn download(client: &reqwest::blocking::Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    trusted_url(url)?;
    let mut bytes = Vec::new();
    client
        .get(url)
        .send()?
        .error_for_status()?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("Download exceeds {limit} bytes")
    }
    Ok(bytes)
}
pub fn root(cache: &Path) -> PathBuf {
    cache.join(VERSION)
}
pub fn sync(cache: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("OptiMC/0.1")
        .build()?;
    let manifest: Manifest =
        serde_json::from_slice(&download(&client, MANIFEST, 8 * 1024 * 1024)?)?;
    let version = manifest
        .versions
        .into_iter()
        .find(|v| v.id == VERSION)
        .context("26.2 is absent from Mojang manifest")?;
    let metadata = download(&client, &version.url, 4 * 1024 * 1024)?;
    checked(&metadata, &version.sha1)?;
    let meta: Metadata = serde_json::from_slice(&metadata)?;
    let dir = root(cache);
    fs::create_dir_all(&dir)?;
    let jar = dir.join("client.jar");
    if !jar.exists() || checked(&fs::read(&jar)?, &meta.downloads.client.sha1).is_err() {
        println!("Downloading official Minecraft {VERSION} assets…");
        let bytes = download(
            &client,
            &meta.downloads.client.url,
            meta.downloads.client.size,
        )?;
        if bytes.len() as u64 != meta.downloads.client.size {
            bail!("Incomplete client archive")
        }
        checked(&bytes, &meta.downloads.client.sha1)?;
        let tmp = dir.join("client.jar.part");
        fs::File::create(&tmp)?.write_all(&bytes)?;
        if jar.exists() {
            fs::remove_file(&jar)?;
        }
        fs::rename(tmp, &jar)?;
    }
    import_jar(&jar, cache)?;
    let index = download(&client, &meta.asset_index.url, 8 * 1024 * 1024)?;
    checked(&index, &meta.asset_index.sha1)?;
    let index: serde_json::Value = serde_json::from_slice(&index)?;
    for face in 0..6 {
        let name = format!("panorama_{face}.png");
        let entry = &index["objects"][format!("minecraft/textures/gui/title/background/{name}")];
        let hash = entry["hash"]
            .as_str()
            .context("Missing panorama resource hash")?;
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("Invalid panorama hash");
        }
        let path = dir.join("gui").join(name);
        if path.exists() && checked(&fs::read(&path)?, hash).is_ok() {
            continue;
        }
        let bytes = download(
            &client,
            &format!(
                "https://resources.download.minecraft.net/{}/{hash}",
                &hash[..2]
            ),
            8 * 1024 * 1024,
        )?;
        checked(&bytes, hash)?;
        fs::write(path, bytes)?;
    }
    catalog::sync_all(&client, &index, &dir)?;
    fs::write(dir.join("metadata.json"), metadata)?;
    Ok(())
}
/// Imports resources only; never executes or links Minecraft's Java code.
pub fn import_jar(jar: &Path, cache: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(jar)?)?;
    let version: serde_json::Value = serde_json::from_reader(zip.by_name("version.json")?)?;
    if version["id"] != VERSION || version["protocol_version"] != PROTOCOL {
        bail!("Expected Minecraft {VERSION}, protocol {PROTOCOL}; archive contains {version}")
    }
    let out = root(cache).join("textures");
    fs::create_dir_all(&out)?;
    for name in TEXTURES {
        let path = format!("assets/minecraft/textures/block/{name}.png");
        let mut bytes = Vec::new();
        zip.by_name(&path)?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            bail!("Texture too large: {path}")
        }
        image::load_from_memory(&bytes).with_context(|| format!("Invalid texture {path}"))?;
        fs::write(out.join(format!("{name}.png")), bytes)?;
    }
    let gui = root(cache).join("gui");
    fs::create_dir_all(&gui)?;
    for (source, target) in GUI_FILES {
        // Keep real asset-index panoramas instead of overwriting with JAR placeholders.
        if target.starts_with("panorama_") && gui.join(target).exists() {
            continue;
        }
        let mut bytes = Vec::new();
        zip.by_name(&format!("assets/minecraft/{source}"))?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            bail!("GUI resource too large: {source}");
        }
        fs::write(gui.join(target), bytes)?;
    }
    catalog::extract(&mut zip, &root(cache))?;
    fs::write(
        root(cache).join("version.json"),
        serde_json::to_vec_pretty(&version)?,
    )?;
    println!("Official textures available in {}", out.display());
    Ok(())
}
pub fn ensure_gui(cache: &Path) -> Result<()> {
    if !root(cache)
        .join("resources/jar-resource-count.txt")
        .exists()
        || GUI_FILES
            .iter()
            .any(|(_, name)| !root(cache).join("gui").join(name).exists())
    {
        let jar = root(cache).join("client.jar");
        if !jar.exists() {
            bail!("Missing GUI assets; run `optimc assets sync` or import the official 26.2 JAR");
        }
        import_jar(&jar, cache)?;
    }
    Ok(())
}
pub fn panorama_pixels(cache: &Path) -> Result<(Vec<u8>, bool)> {
    let mut pixels = Vec::new();
    let mut valid = true;
    for face in [1, 3, 4, 5, 0, 2] {
        let image =
            image::open(root(cache).join("gui").join(format!("panorama_{face}.png")))?.to_rgba8();
        valid &= image.width() > 1;
        pixels.extend_from_slice(
            image::imageops::resize(&image, 512, 512, image::imageops::FilterType::Triangle)
                .as_raw(),
        );
    }
    Ok((pixels, valid))
}
pub fn texture_pixels(cache: &Path) -> Result<Vec<u8>> {
    let mut pixels = Vec::new();
    for name in TEXTURES {
        let path = root(cache).join("textures").join(format!("{name}.png"));
        let source = image::open(&path)
            .with_context(|| format!("Missing {}; run `optimc assets sync`", path.display()))?
            .to_rgba8();
        if source.width() != 16 || source.height() < 16 {
            bail!("Unsupported texture dimensions: {}", path.display())
        }
        // Animated strips use their first 16×16 frame.
        let tile = image::imageops::crop_imm(&source, 0, 0, 16, 16).to_image();
        pixels.extend_from_slice(
            image::imageops::resize(&tile, 64, 64, image::imageops::FilterType::Nearest).as_raw(),
        );
    }
    let skin = image::open(
        root(cache).join("resources/assets/minecraft/textures/entity/player/wide/steve.png"),
    )?
    .to_rgba8();
    anyhow::ensure!(
        skin.dimensions() == (64, 64),
        "Expected official 64×64 player skin"
    );
    pixels.extend_from_slice(skin.as_raw());
    Ok(pixels)
}
