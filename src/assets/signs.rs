//! CPU-exported original sign transforms, dye text colors and armor stand models.
use anyhow::{ensure, Result};
use flate2::read::GzDecoder;
use sha1::{Digest, Sha1};
use std::{io::Read, sync::OnceLock};
pub fn reference() -> Result<&'static serde_json::Value> {
    static REFERENCE: OnceLock<Result<serde_json::Value, String>> = OnceLock::new();
    REFERENCE
        .get_or_init(|| {
            (|| -> Result<_> {
                let mut bytes = Vec::new();
                GzDecoder::new(
                    include_bytes!("../../data/26.2/signs-reference.json.gz").as_slice(),
                )
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 1024 * 1024, "Sign reference exceeds limit");
                let manifest: serde_json::Value =
                    serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
                ensure!(
                    Some(format!("{:x}", Sha1::digest(&bytes)).as_str())
                        == manifest["sha1"]["signs-reference.json"].as_str(),
                    "Sign reference hash mismatch"
                );
                Ok(serde_json::from_slice(&bytes)?)
            })()
            .map_err(|e| format!("{e:#}"))
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
