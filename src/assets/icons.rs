//! Unmodified application icons from the official downloaded asset index.
use anyhow::{ensure, Context, Result};
use std::path::Path;

pub fn read(cache: &Path, name: &str) -> Result<Vec<u8>> {
    ensure!(
        matches!(name, "minecraft.icns" | "icon_32x32.png"),
        "Unknown application icon"
    );
    let root = super::root(cache);
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("asset-index.json"))?)?;
    let key = format!("icons/{name}");
    let object = &index["objects"][&key];
    let hash = object["hash"]
        .as_str()
        .context("Official icon absent from asset index")?;
    let bytes = std::fs::read(root.join("resources/assets").join(&key))?;
    ensure!(
        Some(bytes.len() as u64) == object["size"].as_u64(),
        "Official icon size mismatch"
    );
    super::checked(&bytes, hash)?;
    Ok(bytes)
}

/// Also covers launching the executable directly, outside a Finder app bundle.
#[cfg(target_os = "macos")]
pub fn apply_macos(cache: &Path) -> Result<()> {
    use objc2::ClassType;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};
    let main = MainThreadMarker::new().context("Application icon requires the main thread")?;
    let bytes = read(cache, "minecraft.icns")?;
    let data = NSData::with_bytes(&bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .context("Decoding official application icon")?;
    // NSApplication is accessed on the winit main thread; it retains the image.
    unsafe {
        NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
    }
    Ok(())
}
