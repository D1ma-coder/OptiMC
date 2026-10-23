//! Original-format saved server list; independent of the renderer and JVM.
use crate::net::{
    self,
    nbt::{self, NbtString, Tag},
};
use anyhow::{ensure, Context, Result};
use base64::Engine;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub name: String,
    pub address: String,
    pub extra: BTreeMap<NbtString, Tag>,
}
impl Server {
    pub fn new(name: String, address: String) -> Result<Self> {
        ensure!(
            !name.trim().is_empty() && name.chars().count() <= 128,
            "Введите имя сервера (до 128 символов)"
        );
        net::Address::parse(address.trim())?;
        Ok(Self {
            name,
            address: address.trim().into(),
            extra: BTreeMap::new(),
        })
    }
}
#[derive(Default, Clone)]
pub struct Book {
    pub servers: Vec<Server>,
    root: BTreeMap<NbtString, Tag>,
    root_name: NbtString,
}
pub fn load_username(cache: &Path) -> String {
    std::fs::read(cache.join("profile.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|v| v["username"].as_str().map(str::to_owned))
        .filter(|s| valid_username(s))
        .unwrap_or_else(|| "OptiPlayer".into())
}
pub fn valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 16
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
pub fn save_username(cache: &Path, name: &str) -> Result<()> {
    ensure!(valid_username(name), "Недопустимое имя игрока");
    std::fs::create_dir_all(cache)?;
    let temp = cache.join("profile.json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec(&serde_json::json!({"username": name}))?,
    )?;
    std::fs::rename(temp, cache.join("profile.json"))?;
    Ok(())
}
impl Book {
    pub fn load(cache: &Path) -> Result<Self> {
        let path = cache.join("servers.dat");
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "servers.dat exceeds 1 MiB"
        );
        Self::decode(&std::fs::read(path)?)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        let mut input = bytes;
        let (root_name, tag) = nbt::read_named(&mut input)?;
        ensure!(input.is_empty(), "Trailing servers.dat bytes");
        let Tag::Compound(mut root) = tag else {
            anyhow::bail!("Invalid servers.dat root");
        };
        let mut servers = Vec::new();
        if let Some(list) = root.remove(&"servers".into()) {
            let Tag::List {
                element_type,
                values,
            } = list
            else {
                anyhow::bail!("Invalid server list");
            };
            ensure!(
                element_type == 10 || values.is_empty(),
                "Invalid server entry type"
            );
            ensure!(values.len() <= 1024, "Too many saved servers");
            for tag in values {
                let Tag::Compound(mut extra) = tag else {
                    anyhow::bail!("Invalid server entry");
                };
                let name = extra
                    .remove(&"name".into())
                    .and_then(|t| t.string().map(NbtString::to_string_lossy))
                    .context("Missing server name")?;
                let address = extra
                    .remove(&"ip".into())
                    .and_then(|t| t.string().map(NbtString::to_string_lossy))
                    .context("Missing server address")?;
                servers.push(Server {
                    name,
                    address,
                    extra,
                });
            }
        }
        Ok(Self {
            servers,
            root,
            root_name,
        })
    }
    fn encode(&self) -> Result<Vec<u8>> {
        ensure!(self.servers.len() <= 1024, "Too many saved servers");
        let mut root = self.root.clone();
        root.insert(
            "servers".into(),
            Tag::List {
                element_type: 10,
                values: self
                    .servers
                    .iter()
                    .map(|server| {
                        let mut fields = server.extra.clone();
                        fields.insert("name".into(), Tag::String(server.name.as_str().into()));
                        fields.insert("ip".into(), Tag::String(server.address.as_str().into()));
                        Tag::Compound(fields)
                    })
                    .collect(),
            },
        );
        let mut bytes = Vec::new();
        nbt::write_named(&self.root_name, &Tag::Compound(root), &mut bytes)?;
        ensure!(bytes.len() <= 1024 * 1024, "servers.dat exceeds 1 MiB");
        Ok(bytes)
    }
    pub fn save(&self, cache: &Path) -> Result<()> {
        let bytes = self.encode()?;
        std::fs::create_dir_all(cache)?;
        let path = cache.join("servers.dat");
        let temp = cache.join("servers.dat.tmp");
        std::fs::write(&temp, bytes)?;
        if path.exists() {
            std::fs::copy(&path, cache.join("servers.dat_old"))?;
        }
        std::fs::rename(temp, path)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Ping {
    Waiting,
    Online {
        motd: crate::text::RichText,
        players: String,
        latency: f64,
        icon: Option<(String, Arc<image::RgbaImage>)>,
    },
    Failed(String),
}
pub struct Browser {
    pub book: Book,
    pub selected: Option<usize>,
    pub scroll: usize,
    pub name: String,
    pub editing: Option<usize>,
    pub statuses: BTreeMap<String, Ping>,
    pub icons: BTreeMap<String, Arc<image::RgbaImage>>,
    pub cache: PathBuf,
    pub load_error: Option<String>,
    rx: Option<mpsc::Receiver<(String, Ping)>>,
    stop: Arc<AtomicBool>,
}
impl Browser {
    pub fn visible_indices(&self) -> Vec<usize> {
        self.book
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.extra.get(&"hidden".into()).and_then(Tag::integer) != Some(1))
            .map(|(i, _)| i)
            .collect()
    }
    pub fn navigate(&mut self, down: bool, rows: usize) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected = None;
            self.scroll = 0;
            return;
        }
        let old = self
            .selected
            .and_then(|i| visible.iter().position(|&j| j == i));
        let row = match old {
            None => 0,
            Some(i) if down => (i + 1).min(visible.len() - 1),
            Some(i) => i.saturating_sub(1),
        };
        self.selected = Some(visible[row]);
        if row < self.scroll {
            self.scroll = row;
        }
        if row >= self.scroll.saturating_add(rows.max(1)) {
            self.scroll = row + 1 - rows.max(1);
        }
    }
    pub fn new(cache: &Path) -> Self {
        let (book, load_error) = match Book::load(cache) {
            Ok(b) => (b, None),
            Err(e) => (
                Book::default(),
                Some(format!("Не удалось прочитать servers.dat: {e}")),
            ),
        };
        let icons = book
            .servers
            .iter()
            .filter_map(|s| {
                let encoded = s.extra.get(&"icon".into())?.string()?.to_string_lossy();
                decode_icon(&encoded)
                    .ok()
                    .map(|pixels| (s.address.clone(), pixels))
            })
            .collect();
        Self {
            book,
            selected: None,
            scroll: 0,
            name: String::new(),
            editing: None,
            statuses: BTreeMap::new(),
            icons,
            cache: cache.into(),
            load_error,
            rx: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn save(&self) -> Result<()> {
        ensure!(
            self.load_error.is_none(),
            "Сначала восстановите servers.dat: повреждённый файл не будет перезаписан"
        );
        self.book.save(&self.cache)
    }
    pub fn refresh(&mut self) {
        self.poll();
        if self.rx.is_some() {
            return;
        } // One status worker, including during repeated refresh clicks.
        let addresses: Vec<_> = self
            .book
            .servers
            .iter()
            .filter(|s| s.extra.get(&"hidden".into()).and_then(Tag::integer) != Some(1))
            .map(|s| s.address.clone())
            .collect();
        self.statuses.clear();
        for address in &addresses {
            self.statuses.insert(address.clone(), Ping::Waiting);
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let stop = self.stop.clone();
        std::thread::spawn(move || {
            for address in addresses {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let result = net::Address::parse(&address).and_then(|a| net::status(&a));
                let ping = match result {
                    Ok(v) => Ping::Online {
                        motd: crate::text::RichText::from_json(&v["description"]).unwrap_or_else(
                            |_| crate::text::RichText::plain("Некорректное описание сервера"),
                        ),
                        players: format!("{}/{}", v["players"]["online"], v["players"]["max"]),
                        latency: v["optimc_latency_ms"].as_f64().unwrap_or(0.),
                        icon: v["favicon"].as_str().and_then(|s| {
                            let encoded = s.strip_prefix("data:image/png;base64,")?;
                            decode_icon(encoded)
                                .ok()
                                .map(|pixels| (encoded.to_owned(), pixels))
                        }),
                    },
                    Err(e) => Ping::Failed(e.to_string()),
                };
                if tx.send((address, ping)).is_err() {
                    break;
                }
            }
        });
    }
    pub fn poll(&mut self) {
        let mut changed = false;
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok((address, result)) => {
                        if let Ping::Online { icon, .. } = &result {
                            if let Some((_, pixels)) = icon {
                                self.icons.insert(address.clone(), pixels.clone());
                            } else {
                                self.icons.remove(&address);
                            }
                            for server in self
                                .book
                                .servers
                                .iter_mut()
                                .filter(|s| s.address == address)
                            {
                                let previous = server.extra.remove(&"icon".into());
                                let next = icon
                                    .as_ref()
                                    .map(|(encoded, _)| Tag::String(encoded.as_str().into()));
                                changed |= previous != next;
                                if let Some(icon) = next {
                                    server.extra.insert("icon".into(), icon);
                                }
                            }
                        }
                        self.statuses.insert(address, result);
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.rx = None;
                        break;
                    }
                }
            }
        }
        if changed && self.load_error.is_none() {
            // Status metadata is saved once per received batch, without touching settings.
            if let Err(e) = self.save() {
                self.load_error = Some(format!("Не удалось сохранить иконку сервера: {e}"));
            }
        }
    }
}
pub fn decode_icon(encoded: &str) -> Result<Arc<image::RgbaImage>> {
    ensure!(
        encoded.len() <= 65535,
        "Server icon exceeds NBT string limit"
    );
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(64);
    limits.max_image_height = Some(64);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.to_rgba8();
    ensure!(image.dimensions() == (64, 64), "Server icon must be 64×64");
    Ok(Arc::new(image))
}

/// Same thresholds as ServerSelectionList.OnlineServerEntry in the 26.2 JAR.
pub fn ping_sprite(latency: f64) -> &'static str {
    if latency < 150. {
        "ping_5"
    } else if latency < 300. {
        "ping_4"
    } else if latency < 600. {
        "ping_3"
    } else if latency < 1000. {
        "ping_2"
    } else {
        "ping_1"
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn favicon_png_retains_exact_pixels_and_rejects_invalid_dimensions() {
        let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([30, 90, 180, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let decoded = decode_icon(&encoded).unwrap();
        assert_eq!(*decoded, image);
        assert!(decode_icon("not base64!").is_err());
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(65, 64)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(
            decode_icon(&base64::engine::general_purpose::STANDARD.encode(png.into_inner()))
                .is_err()
        );
        assert_eq!(
            [0., 149., 150., 299., 300., 599., 600., 999., 1000., 1274.].map(ping_sprite),
            [
                "ping_5", "ping_5", "ping_4", "ping_4", "ping_3", "ping_3", "ping_2", "ping_2",
                "ping_1", "ping_1"
            ]
        );
    }
    #[test]
    fn original_named_nbt_preserves_icons_resource_policy_and_unknown_fields() {
        let mut book = Book::default();
        let mut server = Server::new("Сервер".into(), "mclucky.me".into()).unwrap();
        server
            .extra
            .insert("icon".into(), Tag::String("base64 icon".into()));
        server.extra.insert("acceptTextures".into(), Tag::Byte(1));
        server.extra.insert("hidden".into(), Tag::Byte(0));
        book.servers.push(server);
        book.root.insert("extension".into(), Tag::Int(7));
        let bytes = book.encode().unwrap();
        assert_eq!(&bytes[..3], &[10, 0, 0]);
        let mut decoded = Book::decode(&bytes).unwrap();
        assert_eq!(decoded.servers, book.servers);
        assert_eq!(decoded.root, book.root);
        decoded.servers[0].name = "Changed".into();
        assert_eq!(
            Book::decode(&decoded.encode().unwrap()).unwrap().servers[0].extra,
            book.servers[0].extra
        );
        assert!(Book::decode(&bytes[..bytes.len() - 1]).is_err());
    }
    #[test]
    fn save_backup_and_corruption_protection() {
        let cache = std::env::temp_dir().join(format!("optimc-servers-{}", std::process::id()));
        let mut browser = Browser::new(&cache);
        browser
            .book
            .servers
            .push(Server::new("Local".into(), "localhost:25565".into()).unwrap());
        browser.save().unwrap();
        let old = std::fs::read(cache.join("servers.dat")).unwrap();
        browser.book.servers[0].name = "Edited".into();
        browser.save().unwrap();
        assert_eq!(std::fs::read(cache.join("servers.dat_old")).unwrap(), old);
        assert_eq!(Book::load(&cache).unwrap().servers[0].name, "Edited");
        std::fs::write(cache.join("servers.dat"), b"broken").unwrap();
        let broken = Browser::new(&cache);
        assert!(broken.save().is_err());
        assert_eq!(std::fs::read(cache.join("servers.dat")).unwrap(), b"broken");
        std::fs::remove_dir_all(cache).unwrap();
    }
    #[test]
    fn motd_component_keeps_extra_and_strips_legacy_colors() {
        assert_eq!(
            crate::text::RichText::from_json(
                &serde_json::json!({"text":"§aLucky", "extra":[{"text":"\nМир"}]})
            )
            .unwrap()
            .plain_text(),
            "Lucky\nМир"
        );
    }

    #[test]
    fn keyboard_selection_skips_hidden_servers_and_scrolls_visible_rows() {
        let mut browser = Browser::new(&std::env::temp_dir().join("optimc-no-saved-server-list"));
        for i in 0..6 {
            browser
                .book
                .servers
                .push(Server::new(format!("Server {i}"), "localhost".into()).unwrap());
        }
        browser.book.servers[1]
            .extra
            .insert("hidden".into(), Tag::Byte(1));
        browser.navigate(true, 2);
        assert_eq!(browser.selected, Some(0));
        browser.navigate(true, 2);
        assert_eq!(browser.selected, Some(2));
        browser.navigate(true, 2);
        assert_eq!((browser.selected, browser.scroll), (Some(3), 1));
        browser.navigate(false, 2);
        browser.navigate(false, 2);
        assert_eq!((browser.selected, browser.scroll), (Some(0), 0));
    }
}
