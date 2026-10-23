use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Low,
    Medium,
    High,
}
impl Quality {
    pub fn level(self) -> u32 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "Классический вид Minecraft. Максимум FPS.",
            Self::Medium => "Мягкие тени, тёплый свет, облака и атмосфера.",
            Self::High => "Детальные тени, объёмный свет и свечение.",
        }
    }
    pub fn shadow_size(self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2048,
            Self::High => 4096,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub vsync: bool,
    pub fps_limit: u32,
    pub quality: Quality,
    pub render_distance: i32,
    pub view_bobbing: bool,
    pub gui_scale: u32,
    pub render_resolution: u32,
    pub fov: u32,
    pub sensitivity: f64,
    pub invert_x_mouse: bool,
    pub invert_y_mouse: bool,
    pub wheel_sensitivity: f64,
    pub discrete_scroll: bool,
    pub camera_smoothing: bool,
    pub zoom: crate::mods::zoom::Config,
    pub skin_layers: crate::mods::skin_layers::Config,
    pub section_occlusion: bool,
    /// 0 = automatic, 1..999 = aggressive to gentle, 1000 = off.
    pub lod_detail: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            vsync: true,
            fps_limit: 120,
            quality: Quality::Low,
            render_distance: 1024,
            view_bobbing: true,
            gui_scale: 0,
            render_resolution: 1000,
            fov: 70,
            sensitivity: 0.5,
            invert_x_mouse: false,
            invert_y_mouse: false,
            wheel_sensitivity: 1.,
            discrete_scroll: false,
            camera_smoothing: true,
            zoom: Default::default(),
            skin_layers: Default::default(),
            section_occlusion: true,
            lod_detail: 0,
        }
    }
}
impl Settings {
    pub fn load(cache: &Path) -> Self {
        match fs::read(cache.join("settings.json")) {
            Ok(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(mut settings) => {
                    settings.normalize();
                    settings
                }
                Err(error) => {
                    eprintln!("Cannot read graphics settings, using defaults: {error}");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!("Cannot read graphics settings: {error}");
                Self::default()
            }
        }
    }
    pub fn normalize(&mut self) {
        self.lod_detail = self.lod_detail.min(1000);
        self.zoom.normalize();
        self.skin_layers.normalize();
        self.fov = self.fov.clamp(30, 110);
        self.sensitivity = if self.sensitivity.is_finite() {
            self.sensitivity.clamp(0., 1.)
        } else {
            0.5
        };
        self.wheel_sensitivity = if self.wheel_sensitivity.is_finite() {
            self.wheel_sensitivity.clamp(0.01, 10.)
        } else {
            1.
        };
        self.render_resolution = self.render_resolution.min(1000);
        self.gui_scale = self.gui_scale.min(64);
        self.fps_limit = self.fps_limit.min(1000);
        self.render_distance = self.render_distance.clamp(256, 4096);
    }
    pub fn save(&self, cache: &Path) -> Result<()> {
        fs::create_dir_all(cache)?;
        let tmp = cache.join("settings.json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, cache.join("settings.json")).context("Saving graphics settings")?;
        Ok(())
    }
    /// Auto uses scene complexity and pixel count, independent of VSync/FPS caps.
    pub fn lod_distance(&self, triangles: u64, pixels: u64) -> f32 {
        match self.lod_detail {
            1000.. => 0.,
            0 => {
                let load = (triangles as f32 / 250_000. * pixels as f32 / 2_073_600.).max(0.01);
                (192. / load.sqrt()).clamp(64., 256.)
            }
            value => 48. + (value - 1) as f32 / 998. * 336.,
        }
    }
    /// Preserve projected detail during zoom, using perspective focal length.
    /// A non-finite FOV falls back to the reference view instead of poisoning
    /// the shared polygon/shader uniform with NaN.
    pub fn lod_view_distance(&self, triangles: u64, pixels: u64, fov: f32) -> f32 {
        let fov = if fov.is_finite() {
            fov.clamp(1., 179.)
        } else {
            70.
        };
        let magnification = (35f32.to_radians().tan() / (fov.to_radians() * 0.5).tan()).max(1.);
        self.lod_distance(triangles, pixels) * magnification
    }
    pub fn lod_label(&self) -> String {
        match self.lod_detail {
            0 => "LOD: Авто".into(),
            1000.. => "LOD: Выкл".into(),
            n => format!("LOD: {}% качества", (n as f32 / 999. * 100.).round() as u32),
        }
    }
    /// One proportional slider: HD at zero, current display at 1000.
    /// Fit the display to the window aspect ratio; never stretch the scene.
    pub fn render_size(&self, output: [u32; 2], display: [u32; 2], max_dimension: u32) -> [u32; 2] {
        let output = output.map(|v| v.max(1));
        let display = if display.contains(&0) {
            output
        } else {
            display
        };
        let upper = (display[0] as f64 / output[0] as f64)
            .min(display[1] as f64 / output[1] as f64)
            .min(max_dimension as f64 / output[0] as f64)
            .min(max_dimension as f64 / output[1] as f64);
        let lower = (1280. / output[0] as f64)
            .max(720. / output[1] as f64)
            .min(upper);
        let scale = lower + (upper - lower) * self.render_resolution.min(1000) as f64 / 1000.;
        output.map(|size| (size as f64 * scale).round().max(1.) as u32)
    }
    /// MouseHandler -> Entity.turn: cubic sensitivity, degrees to radians.
    pub fn mouse_radians(&self) -> f64 {
        (self.sensitivity * 0.6 + 0.2).powi(3) * 8. * 0.15 * std::f64::consts::PI / 180.
    }
    pub fn frame_interval(&self) -> Option<Duration> {
        (self.fps_limit > 0).then(|| Duration::from_secs_f64(1. / self.fps_limit as f64))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lod_slider_auto_manual_off_and_migration() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.lod_detail, 0);
        let light = settings.lod_distance(10_000, 1280 * 720);
        let heavy = settings.lod_distance(1_000_000, 3840 * 2160);
        assert!(heavy < light && heavy >= 64. && light <= 256.);
        let mut previous = 0.;
        for value in 1..1000 {
            settings.lod_detail = value;
            let distance = settings.lod_distance(0, 0);
            assert!(distance >= previous);
            previous = distance;
        }
        settings.lod_detail = 1000;
        assert_eq!(settings.lod_distance(u64::MAX, u64::MAX), 0.);
        settings.lod_detail = u32::MAX;
        settings.normalize();
        assert_eq!(settings.lod_detail, 1000);
        assert_eq!(
            serde_json::from_str::<Settings>(&serde_json::to_string(&settings).unwrap()).unwrap(),
            settings
        );
    }
    #[test]
    fn shared_lod_preserves_projected_detail_and_off_during_zoom() {
        let mut settings = Settings {
            lod_detail: 500,
            ..Settings::default()
        };
        let base = settings.lod_view_distance(100_000, 1920 * 1080, 70.);
        let zoom = settings.lod_view_distance(100_000, 1920 * 1080, 15.);
        assert!((base * 35f32.to_radians().tan() - zoom * 7.5f32.to_radians().tan()).abs() < 0.001);
        for fov in [110., f32::NAN, f32::INFINITY] {
            assert_eq!(settings.lod_view_distance(100_000, 1920 * 1080, fov), base);
        }
        settings.lod_detail = 1000;
        for fov in [1., 15., 70., f32::NAN] {
            assert_eq!(settings.lod_view_distance(u64::MAX, u64::MAX, fov), 0.);
        }
    }
    #[test]
    fn original_mouse_curve_and_option_bounds() {
        let mut s = Settings::default();
        assert!((s.mouse_radians() - 0.15f64.to_radians()).abs() < 1e-12);
        s.fov = 999;
        s.sensitivity = f64::NAN;
        s.wheel_sensitivity = 0.;
        s.normalize();
        assert_eq!(s.fov, 110);
        assert_eq!(s.sensitivity, 0.5);
        assert_eq!(s.wheel_sensitivity, 0.01);
    }
    #[test]
    fn old_settings_gain_defaults_and_values_are_bounded() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"vsync":false,"fps_limit":99999}"#).unwrap();
        settings.normalize();
        assert!(!settings.vsync);
        assert_eq!(settings.fps_limit, 1000);
        assert_eq!(settings.quality, Quality::Low);
        assert_eq!(
            Settings {
                fps_limit: 0,
                ..settings.clone()
            }
            .frame_interval(),
            None
        );
        assert!(
            (Settings {
                fps_limit: 60,
                ..settings
            }
            .frame_interval()
            .unwrap()
            .as_secs_f64()
                - 1. / 60.)
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn render_resolution_uses_one_proportional_hd_to_display_slider() {
        let mut settings = Settings {
            render_resolution: 0,
            ..Settings::default()
        };
        assert_eq!(
            settings.render_size([1920, 1080], [1920, 1080], 8192),
            [1280, 720]
        );
        settings.render_resolution = 500;
        assert_eq!(
            settings.render_size([1920, 1080], [1920, 1080], 8192),
            [1600, 900]
        );
        settings.render_resolution = 1000;
        assert_eq!(
            settings.render_size([1920, 1080], [1920, 1080], 8192),
            [1920, 1080]
        );
        assert_eq!(
            settings.render_size([1440, 900], [3024, 1964], 8192),
            [3024, 1890]
        );
        assert_eq!(
            settings.render_size([7680, 4320], [15360, 8640], 8192),
            [8192, 4608]
        );
        settings.render_resolution = 0;
        assert_eq!(
            settings.render_size([1440, 900], [3024, 1964], 8192),
            [1280, 800]
        );
        assert_eq!(
            settings.render_size([800, 600], [800, 600], 8192),
            [800, 600]
        );
    }
    #[test]
    fn graphics_settings_survive_restart() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("optimc-settings-{}-{unique}", std::process::id()));
        let settings = Settings {
            vsync: false,
            fps_limit: 60,
            quality: Quality::High,
            gui_scale: 5,
            render_resolution: 500,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        std::fs::remove_dir_all(path).unwrap();
    }
}
