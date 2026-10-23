//! Local offline identities and the public, unauthenticated Ely.by skin service.
use anyhow::{ensure, Context, Result};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub username: String,
    #[serde(default = "enabled")]
    pub ely_skins: bool,
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub profiles: Vec<Profile>,
    pub active: usize,
}
impl Book {
    pub fn load(cache: &Path) -> Result<Self> {
        let path = cache.join("accounts.json");
        if !path.exists() {
            return Ok(Self {
                profiles: vec![Profile {
                    username: crate::multiplayer::load_username(cache),
                    ely_skins: true,
                }],
                active: 0,
            });
        }
        ensure!(
            std::fs::metadata(&path)?.len() <= 65536,
            "Слишком большой файл аккаунтов"
        );
        let book: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        book.validate()?;
        Ok(book)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.profiles.is_empty()
                && self.profiles.len() <= 128
                && self.active < self.profiles.len(),
            "Некорректный список аккаунтов"
        );
        for (i, profile) in self.profiles.iter().enumerate() {
            ensure!(
                crate::multiplayer::valid_username(&profile.username),
                "Ник: 1–16 латинских букв, цифр или _"
            );
            ensure!(
                !self.profiles[..i]
                    .iter()
                    .any(|p| p.username.eq_ignore_ascii_case(&profile.username)),
                "Такой профиль уже существует"
            );
        }
        Ok(())
    }
    fn save(&self, cache: &Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir_all(cache)?;
        let tmp = cache.join("accounts.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, cache.join("accounts.json"))?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct Skin {
    pub pixels: Arc<RgbaImage>,
    pub slim: bool,
}
struct Reply {
    name: String,
    result: Result<Option<Skin>>,
}
pub struct Accounts {
    pub book: Book,
    pub load_error: Option<String>,
    pub status: String,
    pub skin: Option<Skin>,
    cache: PathBuf,
    default: Skin,
    requested: Option<String>,
    completed: Option<String>,
    rx: Option<mpsc::Receiver<Reply>>,
}
impl Accounts {
    pub fn new(cache: &Path) -> Result<Self> {
        let fallback = || Book {
            profiles: vec![Profile {
                username: crate::multiplayer::load_username(cache),
                ely_skins: true,
            }],
            active: 0,
        };
        let (book, load_error) = match Book::load(cache) {
            Ok(b) => (b, None),
            Err(e) => (
                fallback(),
                Some(format!("Не удалось прочитать accounts.json: {e}")),
            ),
        };
        let pixels = image::open(
            crate::assets::root(cache)
                .join("resources/assets/minecraft/textures/entity/player/wide/steve.png"),
        )?
        .to_rgba8();
        let default = Skin {
            pixels: Arc::new(pixels),
            slim: false,
        };
        Ok(Self {
            book,
            load_error,
            status: String::new(),
            skin: Some(default.clone()),
            cache: cache.to_owned(),
            default,
            requested: None,
            completed: None,
            rx: None,
        })
    }
    pub fn active(&self) -> &Profile {
        &self.book.profiles[self.book.active]
    }
    fn commit(&mut self, next: Book) -> Result<()> {
        ensure!(
            self.load_error.is_none(),
            "Файл аккаунтов повреждён; он сохранён без изменений"
        );
        next.save(&self.cache)?;
        self.book = next;
        self.reload();
        Ok(())
    }
    pub fn select(&mut self, index: usize) -> Result<()> {
        let mut next = self.book.clone();
        next.active = index;
        self.commit(next)
    }
    pub fn edit(&mut self, index: Option<usize>, profile: Profile) -> Result<()> {
        let mut next = self.book.clone();
        if let Some(index) = index {
            *next.profiles.get_mut(index).context("Аккаунт не найден")? = profile;
        } else {
            next.profiles.push(profile);
            next.active = next.profiles.len() - 1;
        }
        self.commit(next)
    }
    pub fn delete(&mut self, index: usize) -> Result<()> {
        ensure!(
            self.book.profiles.len() > 1 && index < self.book.profiles.len(),
            "Нужен хотя бы один аккаунт"
        );
        let mut next = self.book.clone();
        next.profiles.remove(index);
        if next.active > index {
            next.active -= 1;
        }
        next.active = next.active.min(next.profiles.len() - 1);
        self.commit(next)
    }
    pub fn reload(&mut self) {
        self.skin = Some(self.default.clone());
        self.completed = None;
        self.status.clear();
    }
    /// One bounded background request at a time; stale results never replace another profile.
    pub fn poll(&mut self) {
        if let Some(reply) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.rx = None;
            self.requested = None;
            if self.active().ely_skins && reply.name.eq_ignore_ascii_case(&self.active().username) {
                self.completed = Some(reply.name);
                match reply.result {
                    Ok(Some(skin)) => {
                        self.skin = Some(skin);
                        self.status = "Скин Ely.by загружен".into();
                    }
                    Ok(None) => self.status = "Скин не найден — стандартный скин".into(),
                    Err(_) => {
                        self.status = "Ely.by недоступен — стандартный или кешированный скин".into()
                    }
                }
            }
        }
        if !self.active().ely_skins {
            self.status = "Стандартный скин".into();
            return;
        }
        let name = self.active().username.to_ascii_lowercase();
        if self.requested.is_some() || self.completed.as_ref() == Some(&name) {
            return;
        }
        self.requested = Some(name.clone());
        self.status = "Загрузка скина Ely.by...".into();
        let cache = self.cache.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let result = fetch_skin(&cache, &name);
            let _ = tx.send(Reply { name, result });
        });
    }
}
fn skin_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(4))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("OptiMC/0.1 (public skins)")
        .build()?)
}
fn public_skin_url(value: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(value)?;
    ensure!(
        matches!(url.host_str(), Some("skinsystem.ely.by" | "ely.by")),
        "Unexpected skin host"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none_or(|p| p == 443),
        "Unexpected skin URL credentials/port"
    );
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Unexpected skin URL scheme"
    );
    // Ely.by currently redirects to an http:// storage URL; request that same
    // official resource over HTTPS instead of downgrading the connection.
    url.set_scheme("https")
        .map_err(|_| anyhow::anyhow!("Invalid skin URL"))?;
    Ok(url)
}
fn public_get(
    client: &reqwest::blocking::Client,
    value: &str,
) -> Result<reqwest::blocking::Response> {
    let mut url = public_skin_url(value)?;
    for _ in 0..4 {
        let response = client.get(url.clone()).send()?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .context("Skin redirect has no location")?
            .to_str()?;
        url = public_skin_url(url.join(location)?.as_str())?;
    }
    anyhow::bail!("Too many skin redirects")
}
fn fetch_skin(cache: &Path, name: &str) -> Result<Option<Skin>> {
    ensure!(
        crate::multiplayer::valid_username(name),
        "Invalid skin nickname"
    );
    let dir = cache.join("ely-skins");
    let png = dir.join(format!("{name}.png"));
    let cached = || -> Result<Option<Skin>> {
        if !png.exists() {
            return Ok(None);
        }
        ensure!(
            std::fs::metadata(&png)?.len() <= 1024 * 1024,
            "Cached skin too large"
        );
        let pixels = decode_skin(&std::fs::read(&png)?)?;
        let slim = std::fs::read_to_string(dir.join(format!("{name}.model")))
            .ok()
            .is_some_and(|s| s == "slim");
        Ok(Some(Skin {
            pixels: Arc::new(pixels),
            slim,
        }))
    };
    let download = || -> Result<Option<Skin>> {
        let client = skin_client()?;
        let response = public_get(
            &client,
            &format!("https://skinsystem.ely.by/skins/{name}.png"),
        )?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response.error_for_status()?;
        ensure!(
            response.content_length().unwrap_or(0) <= 1024 * 1024,
            "Skin exceeds 1 MiB"
        );
        let mut bytes = Vec::new();
        response.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 1024 * 1024, "Skin exceeds 1 MiB");
        let pixels = decode_skin(&bytes)?;
        // Metadata supplies arm width. Never follow the texture URL from the response.
        let slim = (|| -> Result<bool> {
            let response = public_get(
                &client,
                &format!("https://skinsystem.ely.by/textures/{name}"),
            )?
            .error_for_status()?;
            let mut data = Vec::new();
            response.take(65537).read_to_end(&mut data)?;
            ensure!(data.len() <= 65536, "Skin metadata too large");
            let value: serde_json::Value = serde_json::from_slice(&data)?;
            Ok(value["SKIN"]["metadata"]["model"] == "slim")
        })()
        .unwrap_or(false);
        // Cache failure must not discard a valid downloaded skin.
        let _ = (|| -> Result<()> {
            std::fs::create_dir_all(&dir)?;
            let tmp = dir.join(format!("{name}.png.tmp"));
            std::fs::write(&tmp, &bytes)?;
            std::fs::rename(tmp, &png)?;
            std::fs::write(
                dir.join(format!("{name}.model")),
                if slim { "slim" } else { "wide" },
            )?;
            Ok(())
        })();
        Ok(Some(Skin {
            pixels: Arc::new(pixels),
            slim,
        }))
    };
    match download() {
        Ok(skin) => Ok(skin),
        Err(e) => cached()?.map(Some).ok_or(e),
    }
}
/// Java's skin alpha rules and legacy 64×32 mirrored limbs, without image resizing.
pub fn decode_skin(bytes: &[u8]) -> Result<RgbaImage> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(64);
    limits.max_image_height = Some(64);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.to_rgba8();
    ensure!(
        image.width() == 64 && matches!(image.height(), 32 | 64),
        "Expected 64×64 or legacy 64×32 skin"
    );
    let legacy = image.height() == 32;
    let mut out = RgbaImage::new(64, 64);
    image::imageops::replace(&mut out, &image, 0, 0);
    if legacy {
        // source x,y; destination x,y; width,height; mirror X (original copyRect).
        for (sx, sy, dx, dy, w, h) in [
            (4, 16, 20, 48, 4, 4),
            (8, 16, 24, 48, 4, 4),
            (0, 20, 24, 52, 4, 12),
            (4, 20, 20, 52, 4, 12),
            (8, 20, 16, 52, 4, 12),
            (12, 20, 28, 52, 4, 12),
            (44, 16, 36, 48, 4, 4),
            (48, 16, 40, 48, 4, 4),
            (40, 20, 40, 52, 4, 12),
            (44, 20, 36, 52, 4, 12),
            (48, 20, 32, 52, 4, 12),
            (52, 20, 44, 52, 4, 12),
        ] {
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(dx + x, dy + y, *image.get_pixel(sx + w - 1 - x, sy + y));
                }
            }
        }
    }
    let opaque = |image: &mut RgbaImage, x0, y0, x1, y1| {
        for y in y0..y1 {
            for x in x0..x1 {
                image.get_pixel_mut(x, y)[3] = 255;
            }
        }
    };
    opaque(&mut out, 0, 0, 32, 16);
    if legacy && !(0..32).any(|y| (32..64).any(|x| out.get_pixel(x, y)[3] < 128)) {
        for y in 0..32 {
            for x in 32..64 {
                out.get_pixel_mut(x, y)[3] = 0;
            }
        }
    }
    opaque(&mut out, 0, 16, 64, 32);
    opaque(&mut out, 16, 48, 48, 64);
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "optimc-accounts-{}-{unique}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn ely_storage_redirects_are_upgraded_and_other_hosts_rejected() {
        assert_eq!(
            public_skin_url("http://ely.by/storage/skins/a.png")
                .unwrap()
                .as_str(),
            "https://ely.by/storage/skins/a.png"
        );
        for value in [
            "http://127.0.0.1/a",
            "https://other.example/a",
            "https://ely.by@other.example/a",
            "https://ely.by:8443/a",
            "ftp://ely.by/a",
        ] {
            assert!(public_skin_url(value).is_err());
        }
    }
    #[test]
    fn profiles_round_trip_and_migrate() {
        let p = temp();
        crate::multiplayer::save_username(&p, "OldPlayer").unwrap();
        let mut book = Book::load(&p).unwrap();
        assert_eq!(book.profiles[0].username, "OldPlayer");
        book.profiles.push(Profile {
            username: "Other".into(),
            ely_skins: false,
        });
        book.active = 1;
        book.save(&p).unwrap();
        let restored = Book::load(&p).unwrap();
        assert_eq!(restored.active, 1);
        assert_eq!(restored.profiles, book.profiles);
        book.profiles.push(Profile {
            username: "other".into(),
            ely_skins: true,
        });
        assert!(book.save(&p).is_err());
        assert_eq!(Book::load(&p).unwrap().profiles.len(), 2);
        std::fs::remove_dir_all(p).unwrap();
    }
    fn manager(cache: &Path) -> Accounts {
        let path = crate::assets::root(cache)
            .join("resources/assets/minecraft/textures/entity/player/wide/steve.png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        RgbaImage::from_pixel(64, 64, image::Rgba([50, 80, 120, 255]))
            .save(path)
            .unwrap();
        Accounts::new(cache).unwrap()
    }
    #[test]
    fn editing_selection_deletion_and_failed_save_preserve_active_identity() {
        let p = temp();
        let mut accounts = manager(&p);
        accounts
            .edit(
                None,
                Profile {
                    username: "Second".into(),
                    ely_skins: false,
                },
            )
            .unwrap();
        assert_eq!(accounts.active().username, "Second");
        accounts.select(0).unwrap();
        assert_eq!(accounts.active().username, "OptiPlayer");
        accounts
            .edit(
                Some(0),
                Profile {
                    username: "Renamed".into(),
                    ely_skins: true,
                },
            )
            .unwrap();
        assert!(accounts
            .edit(
                Some(0),
                Profile {
                    username: "second".into(),
                    ely_skins: true
                }
            )
            .is_err());
        assert_eq!(accounts.active().username, "Renamed");
        accounts.select(1).unwrap();
        accounts.delete(0).unwrap();
        assert_eq!(accounts.book.active, 0);
        assert_eq!(accounts.active().username, "Second");
        assert!(accounts.delete(0).is_err());
        let restored = manager(&p);
        assert_eq!(restored.active(), accounts.active());
        // An invalid selection must neither change memory nor overwrite the file.
        assert!(accounts.select(99).is_err());
        assert_eq!(Book::load(&p).unwrap().active, 0);
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn manager_protects_a_corrupt_existing_book() {
        let p = temp();
        std::fs::write(p.join("accounts.json"), b"bad").unwrap();
        let mut accounts = manager(&p);
        assert!(accounts.load_error.is_some());
        assert!(accounts
            .edit(
                None,
                Profile {
                    username: "New".into(),
                    ely_skins: true
                }
            )
            .is_err());
        assert_eq!(std::fs::read(p.join("accounts.json")).unwrap(), b"bad");
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    #[ignore = "Public Ely.by HTTPS request; CPU only, no GPU"]
    fn public_ely_skin_download_without_authentication() {
        let p = temp();
        let skin = fetch_skin(&p, "erickskrauch")
            .unwrap()
            .expect("documented public profile");
        assert_eq!(skin.pixels.dimensions(), (64, 64));
        assert!(p.join("ely-skins/erickskrauch.png").exists());
        assert!(!p.join("accounts.json").exists());
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn damaged_file_is_not_replaced() {
        let p = temp();
        std::fs::write(p.join("accounts.json"), b"bad").unwrap();
        assert!(Book::load(&p).is_err());
        assert_eq!(std::fs::read(p.join("accounts.json")).unwrap(), b"bad");
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn skin_limits_legacy_mirroring_and_alpha() {
        use image::ImageEncoder;
        let mut source = RgbaImage::from_pixel(64, 32, image::Rgba([1, 2, 3, 255]));
        source.put_pixel(4, 20, image::Rgba([19, 20, 21, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(source.as_raw(), 64, 32, image::ExtendedColorType::Rgba8)
            .unwrap();
        let skin = decode_skin(&png).unwrap();
        assert_eq!(skin.dimensions(), (64, 64));
        assert_eq!(skin.get_pixel(23, 52)[0], 19);
        assert_eq!(skin.get_pixel(40, 0)[3], 0);
        assert_eq!(skin.get_pixel(8, 8)[3], 255);
        assert!(decode_skin(b"not png").is_err());
    }
}
