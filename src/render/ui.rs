//! Pixel GUI built directly in Rust/WGSL using Mojang's locally imported sprites.
mod accounts;
mod chat;
mod container;
mod debug;
mod hud;
mod menu_input;
mod multiplayer;
mod options;
mod player_list;
mod rich_text;
mod scoreboard;
mod sign_edit;
use crate::{
    assets,
    inventory::{Inventory, Item, Stack},
    settings::{Quality, Settings},
};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use std::{collections::HashMap, path::Path};
use wgpu::util::DeviceExt;
use winit::{
    event::{MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Accounts,
    AccountEdit,
    AccountDelete,
    Playing,
    Chat,
    SignEditor,
    Inventory,
    Container,
    Paused,
    Options,
    Graphics,
    OptiMC,
    MouseSettings,
    ZoomSettings,
    SkinLayerSettings,
    Multiplayer,
    Connecting,
    DirectConnect,
    ServerEdit,
    ConfirmDelete,
    Disconnected,
}
#[derive(Clone, Debug)]
pub enum Action {
    Resume,
    Title,
    Quit,
    Changed,
    Connect(String, String),
    Disconnect,
    Chat(String),
    SignUpdate {
        epoch: u64,
        request: crate::net::request::Request,
    },
    Suggest {
        id: i32,
        input: String,
    },
    CloseMenu {
        id: i32,
        serial: u64,
    },
    MenuClick {
        id: i32,
        serial: u64,
        state: i32,
        click: crate::net::menu_click::Click,
    },
}
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
struct UiVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    atlas: u32,
}
#[derive(Clone, Copy)]
struct Sprite {
    uv: [f32; 4],
    size: [f32; 2],
}
#[derive(Clone, Copy)]
struct Glyph {
    sprite: Sprite,
    advance: f32,
    offset: f32,
}
#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Rect {
    fn contains(self, p: [f32; 2]) -> bool {
        p[0] >= self.x && p[0] < self.x + self.w && p[1] >= self.y && p[1] < self.y + self.h
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Fps,
    Resolution,
    Fov,
    Sensitivity,
    WheelSensitivity,
    ZoomAmount,
    ZoomInTime,
    ZoomOutTime,
    ZoomSteps,
    ZoomStepAmount,
    ZoomSmoothness,
    ZoomSensitivity,
    SkinVoxelSize,
    SkinHeadSize,
    SkinBodySize,
    SkinFirstPersonSize,
    LodDetail,
    SkinDistance,
}
pub struct Ui {
    pub screen: Screen,
    pub in_world: bool,
    pub message: String,
    pub(super) present_mode: wgpu::PresentMode,
    pub rich_reason: Option<crate::text::RichText>,
    pub server_messages: std::sync::Arc<std::collections::VecDeque<crate::net::ServerMessage>>,
    pub server: String,
    pub username: String,
    pub accounts: crate::accounts::Accounts,
    account_selected: usize,
    account_scroll: usize,
    account_edit: Option<usize>,
    account_name: String,
    account_ely: bool,
    account_submit: bool,
    pub inventory: Inventory,
    pub server_inventory: Option<std::sync::Arc<crate::net::container::Store>>,
    item_icons: crate::assets::item_icons::Worker,
    missing_item: std::sync::Arc<image::RgbaImage>,
    close_menu: Option<(i32, u64)>,
    menu_input: menu_input::Input,
    menu_prediction: Option<(u64, std::time::Instant)>,
    menu_preview: Option<std::sync::Arc<crate::net::container::Store>>,
    control: bool,
    pub creative_inventory: bool,
    item_highlight: super::item_highlight::Highlight,
    pub hide_hud: bool,
    pub vitals: crate::net::vitals::Vitals,
    pub server_scoreboard: std::sync::Arc<crate::net::scoreboard::Store>,
    pub server_players: std::sync::Arc<crate::net::player_info::Store>,
    pub server_commands: std::sync::Arc<crate::net::commands::Store>,
    chat: chat::State,
    sign_edit: sign_edit::State,
    tab_held: bool,
    tab_cache: Option<player_list::Cache>,
    sidebar_cache: Option<scoreboard::SidebarCache>,
    pub server_hud: std::sync::Arc<crate::net::hud::State>,
    pub survival_hud: bool,
    pub debug_overlay: bool,
    pub debug_left: Vec<String>,
    pub debug_right: Vec<String>,
    pub fps: f32,
    pub last_draws: usize,
    pub last_triangles: u64,
    pub display_size: [u32; 2],
    shift: bool,
    mouse: [f32; 2],
    pointer: [f32; 2],
    click: Option<MouseButton>,
    held: bool,
    drag: Option<Drag>,
    focus: bool,
    username_focus: bool,
    connect_requested: bool,
    browser: crate::multiplayer::Browser,
    name_focus: bool,
    delete_requested: bool,
    last_server_click: Option<(usize, std::time::Instant)>,
    reason_scroll: usize,
    width: f32,
    height: f32,
    scale: f32,
    sprites: HashMap<String, Sprite>,
    glyphs: HashMap<char, Glyph>,
    vertices: Vec<UiVertex>,
    previous_vertices: Vec<UiVertex>,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    buffer: Option<wgpu::Buffer>,
    count: u32,
    favicon_texture: wgpu::Texture,
    queue: wgpu::Queue,
    favicon_slots: HashMap<String, (usize, std::sync::Arc<image::RgbaImage>)>,
    next_favicon_slot: usize,
}
struct Atlas {
    atlas: image::RgbaImage,
    sprites: HashMap<String, Sprite>,
    glyphs: HashMap<char, Glyph>,
}
impl Ui {
    fn load_atlas(cache: &Path) -> Result<Atlas> {
        let dir = assets::root(cache).join("gui");
        let mut atlas = image::RgbaImage::new(2048, 2048);
        let mut sprites = HashMap::new();
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        let mut sources = HashMap::new();
        for (_, name) in assets::GUI_FILES {
            if !name.ends_with(".png") || name.starts_with("panorama_") {
                continue;
            }
            let image = image::open(dir.join(name))?.to_rgba8();
            if x + image.width() + 2 > 2048 {
                x = 0;
                y += shelf + 2;
                shelf = 0;
            }
            anyhow::ensure!(y + image.height() + 2 < 2048, "GUI atlas overflow");
            image::imageops::replace(&mut atlas, &image, x as i64, y as i64);
            sprites.insert(
                name.to_owned(),
                Sprite {
                    uv: [
                        x as f32 / 2048.,
                        y as f32 / 2048.,
                        (x + image.width()) as f32 / 2048.,
                        (y + image.height()) as f32 / 2048.,
                    ],
                    size: [image.width() as f32, image.height() as f32],
                },
            );
            sources.insert(name.to_owned(), image);
            x += sprites[name].size[0] as u32 + 2;
            shelf = shelf.max(sprites[name].size[1] as u32);
        }
        for (name, path) in [
            ("sign_editor_plain_oak", "gui/signs/oak.png"),
            ("sign_editor_hanging_oak", "gui/hanging_signs/oak.png"),
            ("sign_editor_plain_spruce", "gui/signs/spruce.png"),
            ("sign_editor_hanging_spruce", "gui/hanging_signs/spruce.png"),
            ("sign_editor_plain_birch", "gui/signs/birch.png"),
            ("sign_editor_hanging_birch", "gui/hanging_signs/birch.png"),
            ("sign_editor_plain_jungle", "gui/signs/jungle.png"),
            ("sign_editor_hanging_jungle", "gui/hanging_signs/jungle.png"),
            ("sign_editor_plain_acacia", "gui/signs/acacia.png"),
            ("sign_editor_hanging_acacia", "gui/hanging_signs/acacia.png"),
            ("sign_editor_plain_dark_oak", "gui/signs/dark_oak.png"),
            (
                "sign_editor_hanging_dark_oak",
                "gui/hanging_signs/dark_oak.png",
            ),
            ("sign_editor_plain_mangrove", "gui/signs/mangrove.png"),
            (
                "sign_editor_hanging_mangrove",
                "gui/hanging_signs/mangrove.png",
            ),
            ("sign_editor_plain_cherry", "gui/signs/cherry.png"),
            ("sign_editor_hanging_cherry", "gui/hanging_signs/cherry.png"),
            ("sign_editor_plain_pale_oak", "gui/signs/pale_oak.png"),
            (
                "sign_editor_hanging_pale_oak",
                "gui/hanging_signs/pale_oak.png",
            ),
            ("sign_editor_plain_bamboo", "gui/signs/bamboo.png"),
            ("sign_editor_hanging_bamboo", "gui/hanging_signs/bamboo.png"),
            ("sign_editor_plain_crimson", "gui/signs/crimson.png"),
            (
                "sign_editor_hanging_crimson",
                "gui/hanging_signs/crimson.png",
            ),
            ("sign_editor_plain_warped", "gui/signs/warped.png"),
            ("sign_editor_hanging_warped", "gui/hanging_signs/warped.png"),
            ("inventory", "gui/container/inventory.png"),
            ("container_generic", "gui/container/generic_54.png"),
            ("container_hopper", "gui/container/hopper.png"),
            ("container_generic_3x3", "gui/container/dispenser.png"),
            ("container_shulker", "gui/container/shulker_box.png"),
            ("container_crafting", "gui/container/crafting_table.png"),
            ("container_anvil", "gui/container/anvil.png"),
            ("container_grindstone", "gui/container/grindstone.png"),
            (
                "container_cartography_table",
                "gui/container/cartography_table.png",
            ),
            ("container_stonecutter", "gui/container/stonecutter.png"),
            ("container_crafter_3x3", "gui/container/crafter.png"),
            ("steve", "entity/player/wide/steve.png"),
            ("offhand", "gui/sprites/hud/hotbar_offhand_left.png"),
            ("hotbar", "gui/sprites/hud/hotbar.png"),
            ("selection", "gui/sprites/hud/hotbar_selection.png"),
            ("crosshair", "gui/sprites/hud/crosshair.png"),
            ("heart", "gui/sprites/hud/heart/full.png"),
            ("heart_half", "gui/sprites/hud/heart/half.png"),
            ("heart_empty", "gui/sprites/hud/heart/container.png"),
            ("food_full", "gui/sprites/hud/food_full.png"),
            ("food_half", "gui/sprites/hud/food_half.png"),
            ("food_empty", "gui/sprites/hud/food_empty.png"),
            ("unknown_server", "misc/unknown_server.png"),
            ("ping_1", "gui/sprites/server_list/ping_1.png"),
            ("ping_2", "gui/sprites/server_list/ping_2.png"),
            ("ping_3", "gui/sprites/server_list/ping_3.png"),
            ("ping_4", "gui/sprites/server_list/ping_4.png"),
            ("ping_5", "gui/sprites/server_list/ping_5.png"),
            (
                "ping_unreachable",
                "gui/sprites/server_list/unreachable.png",
            ),
            ("ping_waiting", "gui/sprites/server_list/pinging_1.png"),
            ("tab_ping_unknown", "gui/sprites/icon/ping_unknown.png"),
            ("tab_ping_1", "gui/sprites/icon/ping_1.png"),
            ("tab_ping_2", "gui/sprites/icon/ping_2.png"),
            ("tab_ping_3", "gui/sprites/icon/ping_3.png"),
            ("tab_ping_4", "gui/sprites/icon/ping_4.png"),
            ("tab_ping_5", "gui/sprites/icon/ping_5.png"),
            ("diamond_sword.png", "item/diamond_sword.png"),
            ("iron_pickaxe.png", "item/iron_pickaxe.png"),
            ("apple.png", "item/apple.png"),
            ("bread.png", "item/bread.png"),
            ("grass_top", "block/grass_block_top.png"),
            ("grass_side", "block/grass_block_side.png"),
            ("dirt", "block/dirt.png"),
            ("stone", "block/stone.png"),
            ("sand", "block/sand.png"),
            ("log", "block/oak_log.png"),
            ("log_top", "block/oak_log_top.png"),
            (
                "boss_pink_background",
                "gui/sprites/boss_bar/pink_background.png",
            ),
            (
                "boss_pink_progress",
                "gui/sprites/boss_bar/pink_progress.png",
            ),
            (
                "boss_blue_background",
                "gui/sprites/boss_bar/blue_background.png",
            ),
            (
                "boss_blue_progress",
                "gui/sprites/boss_bar/blue_progress.png",
            ),
            (
                "boss_red_background",
                "gui/sprites/boss_bar/red_background.png",
            ),
            ("boss_red_progress", "gui/sprites/boss_bar/red_progress.png"),
            (
                "boss_green_background",
                "gui/sprites/boss_bar/green_background.png",
            ),
            (
                "boss_green_progress",
                "gui/sprites/boss_bar/green_progress.png",
            ),
            (
                "boss_yellow_background",
                "gui/sprites/boss_bar/yellow_background.png",
            ),
            (
                "boss_yellow_progress",
                "gui/sprites/boss_bar/yellow_progress.png",
            ),
            (
                "boss_purple_background",
                "gui/sprites/boss_bar/purple_background.png",
            ),
            (
                "boss_purple_progress",
                "gui/sprites/boss_bar/purple_progress.png",
            ),
            (
                "boss_white_background",
                "gui/sprites/boss_bar/white_background.png",
            ),
            (
                "boss_white_progress",
                "gui/sprites/boss_bar/white_progress.png",
            ),
            (
                "boss_notched_6_background",
                "gui/sprites/boss_bar/notched_6_background.png",
            ),
            (
                "boss_notched_6_progress",
                "gui/sprites/boss_bar/notched_6_progress.png",
            ),
            (
                "boss_notched_10_background",
                "gui/sprites/boss_bar/notched_10_background.png",
            ),
            (
                "boss_notched_10_progress",
                "gui/sprites/boss_bar/notched_10_progress.png",
            ),
            (
                "boss_notched_12_background",
                "gui/sprites/boss_bar/notched_12_background.png",
            ),
            (
                "boss_notched_12_progress",
                "gui/sprites/boss_bar/notched_12_progress.png",
            ),
            (
                "boss_notched_20_background",
                "gui/sprites/boss_bar/notched_20_background.png",
            ),
            (
                "boss_notched_20_progress",
                "gui/sprites/boss_bar/notched_20_progress.png",
            ),
            (
                "xp_background",
                "gui/sprites/hud/experience_bar_background.png",
            ),
            ("xp_progress", "gui/sprites/hud/experience_bar_progress.png"),
        ] {
            let path = assets::root(cache)
                .join("resources/assets/minecraft/textures")
                .join(path);
            let mut image = image::open(&path)
                .with_context(|| format!("Loading {}", path.display()))?
                .to_rgba8();
            if name.starts_with("container_") {
                let height = if name == "container_generic" {
                    222
                } else if name == "container_hopper" {
                    133
                } else {
                    166
                };
                image = image::imageops::crop_imm(&image, 0, 0, 176, height).to_image();
            }
            if name == "inventory" {
                image = image::imageops::crop_imm(&image, 0, 0, 176, 166).to_image();
            }
            if x + image.width() + 2 > 2048 {
                x = 0;
                y += shelf + 2;
                shelf = 0;
            }
            anyhow::ensure!(y + image.height() + 2 < 2048, "GUI atlas overflow");
            image::imageops::replace(&mut atlas, &image, x as i64, y as i64);
            sprites.insert(
                name.into(),
                Sprite {
                    uv: [
                        x as f32 / 2048.,
                        y as f32 / 2048.,
                        (x + image.width()) as f32 / 2048.,
                        (y + image.height()) as f32 / 2048.,
                    ],
                    size: [image.width() as f32, image.height() as f32],
                },
            );
            x += image.width() + 2;
            shelf = shelf.max(image.height());
        }
        atlas.put_pixel(2047, 2047, image::Rgba([255; 4]));
        let mut glyphs = HashMap::new();
        let font: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("font.json"))?)?;
        for provider in font["providers"]
            .as_array()
            .context("Missing bitmap font providers")?
        {
            if provider["type"] != "bitmap" {
                continue;
            }
            let file = provider["file"]
                .as_str()
                .context("Missing font file")?
                .rsplit('/')
                .next()
                .unwrap();
            let source = sources.get(file).context("Missing font page")?;
            let sprite = sprites[file];
            let rows = provider["chars"].as_array().context("Missing glyph rows")?;
            let columns = rows[0].as_str().unwrap().chars().count();
            let cw = source.width() / columns as u32;
            let ch = source.height() / rows.len() as u32;
            let height = provider["height"].as_f64().unwrap_or(8.) as f32;
            let factor = height / ch as f32;
            for (row, chars) in rows.iter().enumerate() {
                for (col, c) in chars.as_str().unwrap().chars().enumerate() {
                    if c == '\0' || c == ' ' {
                        continue;
                    }
                    let mut ink = 0;
                    for gx in 0..cw {
                        if (0..ch).any(|gy| {
                            source.get_pixel(col as u32 * cw + gx, row as u32 * ch + gy)[3] != 0
                        }) {
                            ink = gx + 1;
                        }
                    }
                    if ink == 0 {
                        continue;
                    }
                    let sx = sprite.uv[0] + col as f32 * cw as f32 / 2048.;
                    let sy = sprite.uv[1] + row as f32 * ch as f32 / 2048.;
                    glyphs.entry(c).or_insert(Glyph {
                        sprite: Sprite {
                            uv: [sx, sy, sx + ink as f32 / 2048., sy + ch as f32 / 2048.],
                            size: [ink as f32 * factor, height],
                        },
                        advance: (ink as f32 * factor + 1.).floor(),
                        offset: 7. - provider["ascent"].as_f64().unwrap_or(7.) as f32,
                    });
                }
            }
        }
        Ok(Atlas {
            atlas,
            sprites,
            glyphs,
        })
    }
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cache: &Path,
    ) -> Result<Self> {
        assets::ensure_gui(cache)?;
        crate::text::load_language(cache);
        let Atlas {
            atlas,
            sprites,
            glyphs,
        } = Self::load_atlas(cache)?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Minecraft GUI atlas"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(2048 * 4),
                rows_per_image: Some(2048),
            },
            wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
        );
        let favicon_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Server favicon cache (1024 original 64×64 icons)"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let favicon_view = favicon_texture.create_view(&Default::default());
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&favicon_view),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Pixel GUI"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GUI overlay"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4,3=>Uint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let accounts = crate::accounts::Accounts::new(cache)?;
        let username = accounts.active().username.clone();
        let account_selected = accounts.book.active;
        Ok(Self {
            screen: Screen::Main,
            in_world: false,
            message: String::new(),
            present_mode: wgpu::PresentMode::Fifo,
            rich_reason: None,
            server_messages: Default::default(),
            server: "localhost:25565".into(),
            username,
            accounts,
            account_selected,
            account_scroll: 0,
            account_edit: None,
            account_name: String::new(),
            account_ely: true,
            account_submit: false,
            browser: crate::multiplayer::Browser::new(cache),
            name_focus: true,
            delete_requested: false,
            last_server_click: None,
            reason_scroll: 0,
            username_focus: false,
            connect_requested: false,
            inventory: Inventory::default(),
            server_inventory: None,
            item_icons: crate::assets::item_icons::Worker::new(cache)?,
            missing_item: std::sync::Arc::new(crate::assets::item_icons::missing()),
            close_menu: None,
            menu_input: Default::default(),
            menu_prediction: None,
            menu_preview: None,
            control: false,
            creative_inventory: false,
            item_highlight: Default::default(),
            hide_hud: false,
            vitals: Default::default(),
            server_hud: Default::default(),
            server_scoreboard: Default::default(),
            server_players: Default::default(),
            server_commands: Default::default(),
            chat: Default::default(),
            sign_edit: Default::default(),
            tab_held: false,
            tab_cache: None,
            sidebar_cache: None,
            survival_hud: true,
            debug_overlay: false,
            debug_left: Vec::new(),
            debug_right: Vec::new(),
            fps: 0.,
            last_draws: 0,
            last_triangles: 0,
            display_size: [0; 2],
            shift: false,
            mouse: [-1.; 2],
            pointer: [-1.; 2],
            click: None,
            held: false,
            drag: None,
            focus: false,
            width: 480.,
            height: 300.,
            scale: 3.,
            sprites,
            glyphs,
            vertices: Vec::new(),
            previous_vertices: Vec::new(),
            pipeline,
            bind,
            buffer: None,
            count: 0,
            favicon_texture,
            queue: queue.clone(),
            favicon_slots: HashMap::new(),
            next_favicon_slot: 0,
        })
    }
    pub fn event(&mut self, event: &WindowEvent) {
        self.menu_event(event);
        if self.screen == Screen::SignEditor {
            self.sign_event(event);
        }
        if self.screen == Screen::Chat {
            self.chat_event(event);
        }
        if matches!(event, WindowEvent::Focused(false)) {
            self.tab_held = false;
        }
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.physical_key == PhysicalKey::Code(KeyCode::Tab) {
                self.tab_held = event.state.is_pressed() && self.screen == Screen::Playing;
            }
            if matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight)
            ) {
                self.shift = event.state.is_pressed();
            }
        }
        match event {
            WindowEvent::Focused(false) => {
                self.shift = false;
                self.held = false;
                self.click = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = [position.x as f32, position.y as f32]
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == MouseButton::Left {
                    self.held = state.is_pressed();
                }
                if state.is_pressed() {
                    self.click = Some(*button);
                }
            }
            WindowEvent::MouseWheel { delta, .. }
                if matches!(
                    self.screen,
                    Screen::Multiplayer | Screen::Disconnected | Screen::Accounts
                ) =>
            {
                let amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.,
                };
                let scroll = if self.screen == Screen::Multiplayer {
                    &mut self.browser.scroll
                } else if self.screen == Screen::Accounts {
                    &mut self.account_scroll
                } else {
                    &mut self.reason_scroll
                };
                if amount > 0. {
                    *scroll = scroll.saturating_sub(amount.ceil() as usize);
                } else {
                    *scroll = scroll.saturating_add((-amount).ceil() as usize);
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && self.screen == Screen::Accounts =>
            {
                let len = self.accounts.book.profiles.len();
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        let result = self.accounts.select(self.account_selected);
                        match result {
                            Ok(()) => {
                                self.username = self.accounts.active().username.clone();
                                self.message.clear();
                            }
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                        self.account_selected = (self.account_selected + 1).min(len - 1)
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        self.account_selected = self.account_selected.saturating_sub(1)
                    }
                    _ => {}
                }
                let rows = ((self.height - 123.) / 24.).floor().max(1.) as usize;
                if self.account_selected < self.account_scroll {
                    self.account_scroll = self.account_selected;
                }
                if self.account_selected >= self.account_scroll + rows {
                    self.account_scroll = self.account_selected + 1 - rows;
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && self.screen == Screen::AccountEdit && self.focus =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        self.account_name.pop();
                    }
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        self.account_submit = true
                    }
                    _ => {
                        if let Some(input) = &event.text {
                            for c in input.chars() {
                                if self.account_name.len() < 16
                                    && (c.is_ascii_alphanumeric() || c == '_')
                                {
                                    self.account_name.push(c);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && self.screen == Screen::Multiplayer =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        self.connect_requested = true
                    }
                    PhysicalKey::Code(KeyCode::F5) => self.browser.refresh(),
                    PhysicalKey::Code(KeyCode::Delete | KeyCode::Backspace) => {
                        self.delete_requested = true
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown | KeyCode::ArrowUp) => {
                        let rows = ((self.height - 93.) / 36.).floor().max(1.) as usize;
                        self.browser.navigate(
                            event.physical_key == PhysicalKey::Code(KeyCode::ArrowDown),
                            rows,
                        );
                    }
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if matches!(self.screen, Screen::DirectConnect | Screen::ServerEdit)
                    && self.focus
                    && event.state.is_pressed() =>
            {
                if matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
                ) {
                    self.connect_requested = true;
                } else if event.physical_key == PhysicalKey::Code(KeyCode::Tab) {
                    if self.screen == Screen::ServerEdit {
                        self.name_focus = !self.name_focus;
                    } else {
                        self.username_focus = !self.username_focus;
                    }
                } else {
                    let nick = self.screen == Screen::DirectConnect && self.username_focus;
                    let text = if nick {
                        &mut self.username
                    } else if self.screen == Screen::ServerEdit && self.name_focus {
                        &mut self.browser.name
                    } else {
                        &mut self.server
                    };
                    if event.physical_key == PhysicalKey::Code(KeyCode::Backspace) {
                        text.pop();
                    } else if let Some(input) = &event.text {
                        for c in input.chars().filter(|c| !c.is_control()) {
                            if text.chars().count() < if nick { 16 } else { 128 }
                                && (!nick || c.is_ascii_alphanumeric() || c == '_')
                            {
                                text.push(c);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    pub fn escape(&mut self) {
        self.screen = match self.screen {
            Screen::Chat => Screen::Playing,
            Screen::SignEditor => {
                self.finish_sign_edit();
                Screen::Playing
            }
            Screen::Container => {
                self.request_menu_close();
                Screen::Playing
            }
            Screen::Inventory => {
                if self.server_inventory.is_none() {
                    self.inventory.close();
                } else {
                    self.request_menu_close();
                }
                Screen::Playing
            }
            Screen::Playing => Screen::Paused,
            Screen::Paused => Screen::Playing,
            Screen::Graphics
            | Screen::OptiMC
            | Screen::MouseSettings
            | Screen::ZoomSettings
            | Screen::SkinLayerSettings => Screen::Options,
            Screen::Options => {
                if self.in_world {
                    Screen::Paused
                } else {
                    Screen::Main
                }
            }
            Screen::Multiplayer | Screen::Accounts => Screen::Main,
            Screen::AccountEdit | Screen::AccountDelete => Screen::Accounts,
            Screen::Connecting
            | Screen::DirectConnect
            | Screen::ServerEdit
            | Screen::ConfirmDelete
            | Screen::Disconnected => Screen::Multiplayer,
            Screen::Main => Screen::Main,
        };
    }
    fn quad(&mut self, rect: Rect, uv: [f32; 4], color: [f32; 4]) {
        let p = [
            [rect.x, rect.y],
            [rect.x + rect.w, rect.y],
            [rect.x + rect.w, rect.y + rect.h],
            [rect.x, rect.y + rect.h],
        ];
        let uvs = [
            [uv[0], uv[1]],
            [uv[2], uv[1]],
            [uv[2], uv[3]],
            [uv[0], uv[3]],
        ];
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(UiVertex {
                position: [
                    p[i][0] / self.width * 2. - 1.,
                    1. - p[i][1] / self.height * 2.,
                ],
                uv: uvs[i],
                color,
                atlas: 0,
            });
        }
    }
    fn fill(&mut self, rect: Rect, color: [f32; 4]) {
        let t = 2047.5 / 2048.;
        self.quad(rect, [t, t, t, t], color);
    }
    fn image(&mut self, name: &str, rect: Rect) {
        self.quad(rect, self.sprites[name].uv, [1.; 4]);
    }
    fn text_width(&self, text: &str) -> f32 {
        text.chars()
            .map(|c| {
                if c == ' ' {
                    4.
                } else {
                    self.glyphs.get(&c).map_or(6., |g| g.advance)
                }
            })
            .sum()
    }
    fn text(&mut self, text: &str, mut x: f32, y: f32, color: [f32; 4]) {
        for c in text.chars() {
            if c == ' ' {
                x += 4.;
                continue;
            }
            if let Some(glyph) = self
                .glyphs
                .get(&c)
                .copied()
                .or_else(|| self.glyphs.get(&'?').copied())
            {
                let r = Rect {
                    x,
                    y: y + glyph.offset,
                    w: glyph.sprite.size[0],
                    h: glyph.sprite.size[1],
                };
                self.quad(
                    Rect {
                        x: r.x + 1.,
                        y: r.y + 1.,
                        ..r
                    },
                    glyph.sprite.uv,
                    [0., 0., 0., color[3] * 0.8],
                );
                self.quad(r, glyph.sprite.uv, color);
                x += glyph.advance;
            }
        }
    }
    fn centered(&mut self, text: &str, y: f32, color: [f32; 4]) {
        let x = (self.width - self.text_width(text)) * 0.5;
        self.text(text, x.round(), y, color);
    }
    fn button(&mut self, text: &str, rect: Rect, enabled: bool) -> bool {
        let hover = rect.contains(self.mouse);
        let name = if !enabled {
            "button_disabled.png"
        } else if hover {
            "button_hover.png"
        } else {
            "button.png"
        };
        // Nine-slice preserves pixel borders at any button width.
        let sprite = self.sprites[name];
        let border = 3.;
        for row in 0..3 {
            for col in 0..3 {
                let edges_x = [0., border, rect.w - border, rect.w];
                let edges_y = [0., border, rect.h - border, rect.h];
                let ux = [
                    sprite.uv[0],
                    sprite.uv[0] + border / 2048.,
                    sprite.uv[2] - border / 2048.,
                    sprite.uv[2],
                ];
                let uy = [
                    sprite.uv[1],
                    sprite.uv[1] + border / 2048.,
                    sprite.uv[3] - border / 2048.,
                    sprite.uv[3],
                ];
                self.quad(
                    Rect {
                        x: rect.x + edges_x[col],
                        y: rect.y + edges_y[row],
                        w: edges_x[col + 1] - edges_x[col],
                        h: edges_y[row + 1] - edges_y[row],
                    },
                    [ux[col], uy[row], ux[col + 1], uy[row + 1]],
                    [1.; 4],
                );
            }
        }
        let color = if enabled {
            [1.; 4]
        } else {
            [0.55, 0.55, 0.55, 1.]
        };
        self.text(
            text,
            (rect.x + (rect.w - self.text_width(text)) * 0.5).round(),
            rect.y + 6.,
            color,
        );
        enabled && hover && self.click == Some(MouseButton::Left)
    }
    fn item(&mut self, stack: Stack, x: f32, y: f32) {
        let key = if let Item::Registry(id) = stack.item {
            Some(crate::assets::item_icons::Key::plain(id))
        } else {
            None
        };
        self.item_with_key(stack, x, y, key);
    }
    fn server_item(&mut self, stack: &crate::net::items::Stack, x: f32, y: f32) {
        self.item_with_key(
            Stack {
                item: Item::Registry(stack.item),
                count: stack.count,
            },
            x,
            y,
            Some(crate::assets::item_icons::Key::stack(stack)),
        );
    }
    fn displayed_local_stack(
        &self,
        local: usize,
    ) -> Option<std::sync::Arc<crate::net::items::Stack>> {
        self.menu_preview
            .as_ref()
            .or(self.server_inventory.as_ref())
            .and_then(|s| {
                (0..46)
                    .find(|&i| crate::net::container::local_slot(i) == Some(local))
                    .and_then(|i| s.player.get(i))
                    .cloned()
            })
    }
    fn local_item(&mut self, local: usize, stack: Stack, x: f32, y: f32) {
        let raw = self
            .menu_preview
            .as_ref()
            .or(self.server_inventory.as_ref())
            .and_then(|s| {
                (0..46)
                    .find(|&i| crate::net::container::local_slot(i) == Some(local))
                    .and_then(|i| s.player.get(i))
                    .cloned()
            });
        if let Some(raw) = raw {
            self.server_item(&raw, x, y);
        } else {
            self.item(stack, x, y);
        }
    }
    fn item_with_key(
        &mut self,
        stack: Stack,
        x: f32,
        y: f32,
        key: Option<crate::assets::item_icons::Key>,
    ) {
        let rect = Rect {
            x,
            y,
            w: 16.,
            h: 16.,
        };
        let block = match stack.item {
            Item::Grass => Some(("grass_top", "grass_side", [0.46, 0.77, 0.27, 1.])),
            Item::Dirt => Some(("dirt", "dirt", [1.; 4])),
            Item::Stone => Some(("stone", "stone", [1.; 4])),
            Item::Sand => Some(("sand", "sand", [1.; 4])),
            Item::Log => Some(("log_top", "log", [1.; 4])),
            _ => None,
        };
        if let Item::Registry(_) = stack.item {
            let pixels = self
                .item_icons
                .get(key.as_ref().expect("registry icon key"))
                .unwrap_or_else(|| self.missing_item.clone());
            self.dynamic_image(
                &key.as_ref().unwrap().cache_name(),
                pixels,
                rect,
                [0., 0., 1., 1.],
            );
        } else if let Some((top, side, tint)) = block {
            self.icon_face(
                [
                    [x + 8., y],
                    [x + 16., y + 4.],
                    [x + 8., y + 8.],
                    [x, y + 4.],
                ],
                top,
                tint,
            );
            self.icon_face(
                [
                    [x, y + 4.],
                    [x + 8., y + 8.],
                    [x + 8., y + 16.],
                    [x, y + 12.],
                ],
                side,
                [0.6, 0.6, 0.6, 1.],
            );
            self.icon_face(
                [
                    [x + 8., y + 8.],
                    [x + 16., y + 4.],
                    [x + 16., y + 12.],
                    [x + 8., y + 16.],
                ],
                side,
                [0.8, 0.8, 0.8, 1.],
            );
        } else {
            self.image(stack.item.icon(), rect);
        }
        if stack.count > 1 {
            let text = stack.count.to_string();
            self.text(&text, x + 17. - self.text_width(&text), y + 10., [1.; 4]);
        }
    }
    fn icon_face(&mut self, points: [[f32; 2]; 4], name: &str, tint: [f32; 4]) {
        let uv = self.sprites[name].uv;
        let uvs = [
            [uv[0], uv[1]],
            [uv[2], uv[1]],
            [uv[2], uv[3]],
            [uv[0], uv[3]],
        ];
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(UiVertex {
                position: [
                    points[i][0] / self.width * 2. - 1.,
                    1. - points[i][1] / self.height * 2.,
                ],
                uv: uvs[i],
                color: tint,
                atlas: 0,
            });
        }
    }
    fn hud(&mut self) {
        self.server_chat();
        if self.survival_hud {
            let left = (self.width * 0.5).floor() - 91.;
            let right = (self.width * 0.5).floor() + 91.;
            let top = self.height - 39.;
            for i in 0..10 {
                let rect = Rect {
                    x: left + i as f32 * 8.,
                    y: top,
                    w: 9.,
                    h: 9.,
                };
                self.image("heart_empty", rect);
                match self.vitals.heart(i) {
                    2 => self.image("heart", rect),
                    1 => self.image("heart_half", rect),
                    _ => {}
                }
                let rect = Rect {
                    x: right - i as f32 * 8. - 9.,
                    ..rect
                };
                self.image("food_empty", rect);
                match self.vitals.hunger(i) {
                    2 => self.image("food_full", rect),
                    1 => self.image("food_half", rect),
                    _ => {}
                }
            }
        }
        if self.debug_overlay {
            self.debug_screen();
        }

        let x = (self.width * 0.5).floor() - 91.;
        let y = self.height - 22.;
        self.image(
            "hotbar",
            Rect {
                x,
                y,
                w: 182.,
                h: 22.,
            },
        );
        self.image(
            "selection",
            Rect {
                x: x - 1. + self.inventory.selected as f32 * 20.,
                y: y - 1.,
                w: 24.,
                h: 23.,
            },
        );
        for i in 0..9 {
            if let Some(stack) = self.inventory.slots[i] {
                self.local_item(i, stack, x + 3. + i as f32 * 20., y + 3.);
            }
        }
        self.image(
            "crosshair",
            Rect {
                x: ((self.width - 15.) * 0.5).floor(),
                y: ((self.height - 15.) * 0.5).floor(),
                w: 15.,
                h: 15.,
            },
        );
        if let Some(stack) = self.inventory.slots[40] {
            self.image(
                "offhand",
                Rect {
                    x: x - 29.,
                    y: y - 1.,
                    w: 29.,
                    h: 24.,
                },
            );
            self.local_item(40, stack, x - 25., y + 3.);
        }
        let selected = self.inventory.selected_stack().map(|stack| {
            let name = self
                .server_stack_name(self.inventory.selected)
                .unwrap_or_else(|| crate::text::RichText::plain(stack.item.name()));
            (stack.item, name)
        });
        let alpha = self
            .item_highlight
            .update(selected.as_ref(), std::time::Instant::now());
        if let Some((_, name)) = selected.filter(|_| alpha > 0.) {
            if let Some(line) = self.styled_lines(&name, self.width - 16.).first() {
                self.centered_styled_line(
                    line,
                    self.height - if self.survival_hud { 59. } else { 45. },
                    [1., 1., 1., alpha],
                );
            }
        }
        self.synchronized_hud();
        self.scoreboard_sidebar();
        if self.tab_held {
            self.player_list();
        }
    }
    fn preview(&mut self, x: f32, y: f32) {
        let player = crate::player::Player::spawn(glam::DVec3::ZERO);
        let slim = self.accounts.skin.as_ref().is_some_and(|skin| skin.slim);
        let mesh = crate::world::avatar::body_model(&player, 0., 0., slim);
        let rotation = glam::Mat4::from_rotation_x(-0.12)
            * glam::Mat4::from_rotation_y(((self.mouse[0] - x) * 0.01).clamp(-0.8, 0.8));
        let mut faces: Vec<_> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| {
                let vertices =
                    [triangle[0], triangle[1], triangle[2]].map(|i| mesh.vertices[i as usize]);
                let points =
                    vertices.map(|v| rotation.transform_point3(glam::Vec3::from_array(v.position)));
                (points.iter().map(|p| p.z).sum::<f32>(), vertices, points)
            })
            .collect();
        faces.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (uv, atlas) = if let Some(skin) = self.accounts.skin.clone() {
            let start = self.vertices.len();
            let sprite = self.dynamic_image(
                "account-skin",
                skin.pixels,
                Rect {
                    x: 0.,
                    y: 0.,
                    w: 0.,
                    h: 0.,
                },
                [0., 0., 1., 1.],
            );
            self.vertices.truncate(start);
            (sprite.uv, 1)
        } else {
            (self.sprites["steve"].uv, 0)
        };
        for (_, vertices, points) in faces {
            let normal = (points[1] - points[0]).cross(points[2] - points[0]);
            if normal.z >= 0. {
                continue;
            }
            let shade = 0.65
                + 0.35
                    * normal
                        .normalize()
                        .dot(glam::Vec3::new(-0.3, 0.8, -0.5).normalize())
                        .max(0.);
            for (v, p) in vertices.into_iter().zip(points) {
                self.vertices.push(UiVertex {
                    position: [
                        (x + p.x * 32.) / self.width * 2. - 1.,
                        1. - (y - p.y * 32.) / self.height * 2.,
                    ],
                    uv: [
                        uv[0] + v.uv[0] * (uv[2] - uv[0]),
                        uv[1] + v.uv[1] * (uv[3] - uv[1]),
                    ],
                    color: [shade, shade, shade, 1.],
                    atlas,
                });
            }
        }
    }
    fn inventory_screen(&mut self) {
        let x = ((self.width - 176.) * 0.5).floor();
        let y = ((self.height - 166.) * 0.5).floor();
        self.image(
            "inventory",
            Rect {
                x,
                y,
                w: 176.,
                h: 166.,
            },
        );
        self.preview(x + 51., y + 75.);
        self.text("Создание", x + 97., y + 6., [0.25, 0.25, 0.25, 1.]);
        let mut hovered = None;
        for i in 0..46 {
            let (sx, sy) = match i {
                0..=8 => (8. + i as f32 * 18., 142.),
                9..=35 => (
                    8. + ((i - 9) % 9) as f32 * 18.,
                    84. + ((i - 9) / 9) as f32 * 18.,
                ),
                36..=39 => (8., 8. + (i - 36) as f32 * 18.),
                40 => (77., 62.),
                41..=44 => (
                    98. + ((i - 41) % 2) as f32 * 18.,
                    18. + ((i - 41) / 2) as f32 * 18.,
                ),
                _ => (154., 28.),
            };
            let rect = Rect {
                x: x + sx,
                y: y + sy,
                w: 16.,
                h: 16.,
            };
            if rect.contains(self.mouse) {
                hovered = Some(i);
                if let Some(button) = self.click.filter(|_| self.server_inventory.is_none()) {
                    self.inventory
                        .click(i, button == MouseButton::Right, self.shift);
                }
            }
            if let Some(raw) = self.displayed_local_stack(i) {
                if raw.count > 0 {
                    self.server_item(&raw, rect.x, rect.y);
                }
            } else if let Some(stack) = self.inventory.slots[i] {
                self.item(stack, rect.x, rect.y);
            }
            if rect.contains(self.mouse) {
                self.fill(rect, [1., 1., 1., 0.25]);
            }
        }
        if let Some(stack) = self.inventory.cursor {
            if let Some(raw) = self
                .menu_preview
                .as_ref()
                .or(self.server_inventory.as_ref())
                .map(|s| s.carried.clone())
            {
                if raw.count > 0 {
                    self.server_item(&raw, self.mouse[0] - 8., self.mouse[1] - 8.);
                }
            } else {
                self.item(stack, self.mouse[0] - 8., self.mouse[1] - 8.);
            }
        } else if let Some(stack) = hovered.and_then(|i| self.inventory.slots[i]) {
            let name = hovered
                .and_then(|i| self.server_stack_name(i))
                .unwrap_or_else(|| crate::text::RichText::plain(stack.item.name()));
            self.item_tooltip(&name, self.mouse[0] + 10., self.mouse[1] - 16.);
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        size: [u32; 2],
        settings: &mut Settings,
    ) -> Vec<Action> {
        let gui = super::gui_scale::GuiScale::calculate(size, settings.gui_scale, false);
        self.scale = gui.factor as f32;
        self.mouse = gui.pointer(self.pointer, size);
        self.width = gui.width as f32;
        self.height = gui.height as f32;
        std::mem::swap(&mut self.vertices, &mut self.previous_vertices);
        self.accounts.poll();
        self.vertices.clear();
        let mut actions = Vec::new();
        self.process_menu_input(gui, size, &mut actions);
        if let Some((epoch, request)) = self.sign_edit.pending.take() {
            actions.push(Action::SignUpdate { epoch, request });
        }
        if let Some((id, serial)) = self.close_menu.take() {
            actions.push(Action::CloseMenu { id, serial });
        }
        let center = (self.width * 0.5).floor();
        let h = self.height;
        let button = |x, y, w| Rect { x, y, w, h: 20. };
        let screen = self.screen;
        if !self.held
            || !matches!(
                screen,
                Screen::Graphics
                    | Screen::OptiMC
                    | Screen::MouseSettings
                    | Screen::ZoomSettings
                    | Screen::SkinLayerSettings
                    | Screen::Options
            )
        {
            self.drag = None;
        }
        if screen != Screen::Playing {
            self.fill(
                Rect {
                    x: 0.,
                    y: 0.,
                    w: self.width,
                    h,
                },
                if self.in_world {
                    [0., 0., 0., 0.52]
                } else {
                    [0.02, 0.03, 0.05, 0.20]
                },
            );
        }
        match screen {
            Screen::Main => {
                let logo = self.sprites["title.png"];
                let width = 256f32.min(self.width - 24.);
                let height = width * logo.size[1] / logo.size[0];
                self.image(
                    "title.png",
                    Rect {
                        x: center - width * 0.5,
                        y: 20.,
                        w: width,
                        h: height,
                    },
                );
                let top = (h * 0.47).max(112.);
                if self.button("Сетевая игра", button(center - 100., top, 200.), true) {
                    self.screen = Screen::Multiplayer;
                    self.message = self.browser.load_error.clone().unwrap_or_default();
                    self.browser.refresh();
                }
                let account_label = format!("Аккаунт: {}", self.accounts.active().username);
                if self.button(&account_label, button(center - 100., top + 24., 200.), true) {
                    self.account_selected = self.accounts.book.active;
                    self.message.clear();
                    self.screen = Screen::Accounts;
                }
                if let Some(skin) = self.accounts.skin.clone() {
                    let r = Rect {
                        x: center - 95.,
                        y: top + 26.,
                        w: 16.,
                        h: 16.,
                    };
                    self.dynamic_image(
                        "account-skin",
                        skin.pixels.clone(),
                        r,
                        [8. / 64., 8. / 64., 16. / 64., 16. / 64.],
                    );
                    self.dynamic_image(
                        "account-skin",
                        skin.pixels,
                        r,
                        [40. / 64., 8. / 64., 48. / 64., 16. / 64.],
                    );
                }
                if self.button("Настройки...", button(center - 100., top + 60., 98.), true)
                {
                    self.screen = Screen::Options;
                }
                if self.button("Выйти из игры", button(center + 2., top + 60., 98.), true)
                {
                    actions.push(Action::Quit);
                }
                self.text("OptiMC 0.1.0 / Minecraft 26.2", 4., h - 12., [1.; 4]);
                let label = "Независимый Rust / Vulkan клиент";
                self.text(
                    label,
                    self.width - self.text_width(label) - 4.,
                    h - 12.,
                    [0.85, 0.85, 0.85, 1.],
                );
            }
            Screen::Accounts | Screen::AccountEdit | Screen::AccountDelete => {
                self.accounts_screen()
            }
            Screen::Paused => {
                self.centered("Меню игры", 40., [1.; 4]);
                if self.button(
                    "Вернуться к игре",
                    button(center - 100., h * 0.4, 200.),
                    true,
                ) {
                    self.screen = Screen::Playing;
                    actions.push(Action::Resume);
                }
                if self.button(
                    "Настройки...",
                    button(center - 100., h * 0.4 + 28., 200.),
                    true,
                ) {
                    self.screen = Screen::Options;
                }
                if self.button(
                    "В главное меню",
                    button(center - 100., h * 0.4 + 64., 200.),
                    true,
                ) {
                    self.in_world = false;
                    self.screen = Screen::Main;
                    actions.push(Action::Title);
                }
            }
            Screen::Options
            | Screen::MouseSettings
            | Screen::ZoomSettings
            | Screen::SkinLayerSettings => self.options_screen(settings, &mut actions),
            Screen::Graphics => {
                self.centered("Настройки графики", 16., [1.; 4]);
                if self.button(
                    &format!("VSync: {}", if settings.vsync { "Вкл" } else { "Выкл" }),
                    button(center - 155., 40., 150.),
                    true,
                ) {
                    settings.vsync = !settings.vsync;
                    actions.push(Action::Changed);
                }
                let fps_rect = button(center + 5., 40., 150.);
                const VALUES: [u32; 10] = [30, 60, 90, 120, 144, 165, 240, 360, 500, 0];
                if self.click == Some(MouseButton::Left) && fps_rect.contains(self.mouse) {
                    self.drag = Some(Drag::Fps);
                }
                if self.held && self.drag == Some(Drag::Fps) {
                    let fraction =
                        ((self.mouse[0] - fps_rect.x - 4.) / (fps_rect.w - 8.)).clamp(0., 1.);
                    let selected = VALUES[(fraction * (VALUES.len() - 1) as f32).round() as usize];
                    if settings.fps_limit != selected {
                        settings.fps_limit = selected;
                        actions.push(Action::Changed);
                    }
                }
                self.button("", fps_rect, true);
                let index = VALUES
                    .iter()
                    .position(|&v| v == settings.fps_limit)
                    .unwrap_or(0);
                let thumb =
                    fps_rect.x + index as f32 / (VALUES.len() - 1) as f32 * (fps_rect.w - 8.);
                self.fill(
                    Rect {
                        x: thumb,
                        y: fps_rect.y,
                        w: 8.,
                        h: 20.,
                    },
                    [0.85, 0.85, 0.85, 1.],
                );
                self.fill(
                    Rect {
                        x: thumb + 1.,
                        y: fps_rect.y + 1.,
                        w: 6.,
                        h: 18.,
                    },
                    [0.28, 0.28, 0.28, 1.],
                );
                let fps = if settings.fps_limit == 0 {
                    "Без лимита".into()
                } else {
                    format!("{}", settings.fps_limit)
                };
                let label = format!("Макс. FPS: {fps}");
                self.text(
                    &label,
                    (fps_rect.x + (fps_rect.w - self.text_width(&label)) * 0.5).round(),
                    fps_rect.y + 6.,
                    [1.; 4],
                );
                let max_scale = super::gui_scale::GuiScale::calculate(size, 0, false).factor;
                let value = if settings.gui_scale == 0 {
                    "Auto".to_owned()
                } else {
                    settings.gui_scale.to_string()
                };
                if self.button(
                    &format!("Масштаб GUI: {value}"),
                    button(center - 155., 66., 310.),
                    true,
                ) {
                    settings.gui_scale = if settings.gui_scale >= max_scale {
                        0
                    } else {
                        settings.gui_scale + 1
                    };
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Покачивание: {}",
                        if settings.view_bobbing {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    button(center - 155., 92., 310.),
                    true,
                ) {
                    settings.view_bobbing = !settings.view_bobbing;
                    actions.push(Action::Changed);
                }
                if let Some(value) = self.option_slider(
                    &settings.lod_label(),
                    button(center - 155., 118., 310.),
                    settings.lod_detail as f32 / 1000.,
                    Drag::LodDetail,
                ) {
                    settings.lod_detail = (value * 1000.).round() as u32;
                    actions.push(Action::Changed);
                }
                self.centered(
                    "Авто ← агрессивнее · качественнее → Выкл",
                    144.,
                    [0.7, 0.7, 0.7, 1.],
                );
                self.centered(
                    super::presentation::description(settings.vsync, self.present_mode),
                    160.,
                    [0.7, 0.7, 0.7, 1.],
                );
                if self.button("Готово", button(center - 100., h - 28., 200.), true) {
                    self.screen = Screen::Options;
                }
            }
            Screen::OptiMC => {
                self.centered("Настройки OptiMC", 16., [1.; 4]);
                let render_rect = button(center - 155., 40., 310.);
                if self.click == Some(MouseButton::Left) && render_rect.contains(self.mouse) {
                    self.drag = Some(Drag::Resolution);
                }
                if self.held && self.drag == Some(Drag::Resolution) {
                    let position = (((self.mouse[0] - render_rect.x - 4.) / (render_rect.w - 8.))
                        .clamp(0., 1.)
                        * 1000.)
                        .round() as u32;
                    if settings.render_resolution != position {
                        settings.render_resolution = position;
                        actions.push(Action::Changed);
                    }
                }
                self.button("", render_rect, true);
                let thumb = render_rect.x
                    + settings.render_resolution as f32 / 1000. * (render_rect.w - 8.);
                self.fill(
                    Rect {
                        x: thumb,
                        y: render_rect.y,
                        w: 8.,
                        h: 20.,
                    },
                    [0.8, 0.8, 0.8, 1.],
                );
                self.fill(
                    Rect {
                        x: thumb + 1.,
                        y: render_rect.y + 1.,
                        w: 6.,
                        h: 18.,
                    },
                    [0.28, 0.28, 0.28, 1.],
                );
                let resolution = settings.render_size(size, self.display_size, 8192);
                self.centered(
                    &format!("Рендер: {}×{}", resolution[0], resolution[1]),
                    46.,
                    [1.; 4],
                );
                self.centered("HD — разрешение экрана", 64., [0.7, 0.7, 0.7, 1.]);
                self.centered("Качество изображения", 80., [1.; 4]);
                for (i, quality) in [Quality::Low, Quality::Medium, Quality::High]
                    .into_iter()
                    .enumerate()
                {
                    let label = if quality == settings.quality {
                        format!("> {} <", quality.label())
                    } else {
                        quality.label().into()
                    };
                    if self.button(
                        &label,
                        button(center - 155. + i as f32 * 105., 96., 100.),
                        true,
                    ) {
                        settings.quality = quality;
                        actions.push(Action::Changed);
                    }
                }
                self.centered(settings.quality.description(), 123., [0.85, 0.85, 0.85, 1.]);
                self.centered(
                    "Medium: стиль BSL   High: стиль Bliss",
                    135.,
                    [0.65, 0.75, 0.85, 1.],
                );
                if self.button(
                    &format!(
                        "Плавная камера: {}",
                        if settings.camera_smoothing {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    button(center - 155., 162., 310.),
                    true,
                ) {
                    settings.camera_smoothing = !settings.camera_smoothing;
                    actions.push(Action::Changed);
                }
                if self.button(
                    &format!(
                        "Отсечение секций: {}",
                        if settings.section_occlusion {
                            "Вкл"
                        } else {
                            "Выкл"
                        }
                    ),
                    button(center - 155., 188., 310.),
                    true,
                ) {
                    settings.section_occlusion = !settings.section_occlusion;
                    actions.push(Action::Changed);
                }
                if !self.message.is_empty() {
                    let message = self.message.clone();
                    self.centered(&message, h - 64., [1., 0.8, 0.4, 1.]);
                }
                if self.button("Готово", button(center - 100., h - 28., 200.), true) {
                    self.screen = Screen::Options;
                }
            }
            Screen::Multiplayer
            | Screen::DirectConnect
            | Screen::ServerEdit
            | Screen::ConfirmDelete
            | Screen::Disconnected => {
                self.multiplayer_screen(&mut actions);
            }
            Screen::Connecting => {
                self.centered("Подключение к серверу", h * 0.5 - 36., [1.; 4]);
                let message = self.message.clone();
                self.centered(&message, h * 0.5 - 12., [0.85, 0.9, 0.85, 1.]);
                if self.button("Отмена", button(center - 100., h - 40., 200.), true) {
                    actions.push(Action::Disconnect);
                }
            }
            Screen::Playing => {
                if !self.hide_hud {
                    self.hud();
                }
            }
            Screen::Chat => {
                self.hud();
                self.chat_screen(&mut actions);
            }
            Screen::Inventory => self.inventory_screen(),
            Screen::Container => self.container_screen(),
            Screen::SignEditor => self.sign_screen(&mut actions),
        }
        self.click = None;
        self.count = self.vertices.len() as u32;
        if self.vertices != self.previous_vertices {
            self.buffer = if self.vertices.is_empty() {
                None
            } else {
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("GUI frame"),
                        contents: bytemuck::cast_slice(&self.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            };
        }
        actions
    }
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if let Some(buffer) = &self.buffer {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Pixel menu"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..self.count, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires Vulkan GPU and imported official GUI resources"]
    fn graphics_controls_apply_and_persist_with_real_gpu_resources() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .unwrap();
        let mut ui = Ui::new(
            &device,
            &queue,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Path::new(".cache"),
        )
        .unwrap();
        let mut settings = Settings::default();
        ui.screen = Screen::Main;
        ui.pointer = [240. * 3., 151. * 3.];
        ui.click = Some(MouseButton::Left);
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(ui.screen, Screen::Multiplayer);
        assert!(!ui.in_world);
        ui.screen = Screen::Graphics;
        ui.pointer = [120. * 3., 50. * 3.];
        ui.click = Some(MouseButton::Left);
        assert!(ui
            .prepare(&device, [1440, 900], &mut settings)
            .iter()
            .any(|a| matches!(a, Action::Changed)));
        assert!(!settings.vsync);
        ui.pointer = [265. * 3., 50. * 3.];
        ui.held = true;
        ui.click = Some(MouseButton::Left);
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(settings.fps_limit, 60);
        ui.held = false;
        ui.screen = Screen::OptiMC;
        ui.pointer = [345. * 3., 106. * 3.];
        ui.click = Some(MouseButton::Left);
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(settings.quality, Quality::High);
        let cache =
            std::env::temp_dir().join(format!("optimc-gpu-ui-settings-{}", std::process::id()));
        settings.save(&cache).unwrap();
        assert_eq!(Settings::load(&cache), settings);
        std::fs::remove_dir_all(cache).unwrap();
        // Switching quality from the live UI must remain possible at a new DPI/size.
        ui.pointer = [0.; 2];
        ui.prepare(&device, [1920, 1080], &mut settings);
        ui.pointer = [(ui.width * 0.5 - 105.) * ui.scale, 106. * ui.scale];
        ui.click = Some(MouseButton::Left);
        ui.prepare(&device, [1920, 1080], &mut settings);
        assert_eq!(settings.quality, Quality::Low);
        settings.gui_scale = 3;
        ui.display_size = [1920, 1080];
        ui.pointer = [(85. + 4.) * 3., 48. * 3.];
        ui.click = Some(MouseButton::Left);
        ui.held = true;
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(settings.render_resolution, 0);
        ui.pointer = [(85. + 310. - 4.) * 3., 48. * 3.];
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(settings.render_resolution, 1000);
        ui.held = false;
        ui.screen = Screen::Inventory;
        ui.in_world = true;
        settings.gui_scale = 3;
        ui.pointer = [(152. + 8. + 8.) * 3., (67. + 142. + 8.) * 3.];
        ui.click = Some(MouseButton::Right);
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(ui.inventory.cursor.unwrap().count, 32);
        ui.pointer = [(152. + 8. + 8.) * 3., (67. + 84. + 8.) * 3.];
        ui.click = Some(MouseButton::Left);
        ui.prepare(&device, [1440, 900], &mut settings);
        assert_eq!(ui.inventory.slots[9].unwrap().count, 32);
        assert!(ui.inventory.cursor.is_none());
        ui.screen = Screen::DirectConnect;
        ui.in_world = false;
        ui.server = "127.0.0.1:25565".into();
        ui.username = "Fixture".into();
        ui.pointer = [240. * 3., 255. * 3.];
        ui.click = Some(MouseButton::Left);
        let actions = ui.prepare(&device, [1440, 900], &mut settings);
        assert!(actions.iter().any(|a| matches!(a,Action::Connect(server,name) if server=="127.0.0.1:25565" && name=="Fixture")));
        assert_eq!(ui.screen, Screen::Connecting);
        ui.pointer = [240. * 3., (300. - 30.) * 3.];
        ui.click = Some(MouseButton::Left);
        let actions = ui.prepare(&device, [1440, 900], &mut settings);
        assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
        ui.escape();
        assert_eq!(ui.screen, Screen::Multiplayer);
    }
}
#[cfg(test)]
mod inventory_atlas_tests {
    use super::*;
    #[test]
    #[ignore = "Requires imported official GUI resources; CPU only"]
    fn inventory_and_container_assets_fit_the_real_atlas_without_a_gpu() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        assets::ensure_gui(&cache).unwrap();
        let atlas = Ui::load_atlas(&cache).unwrap();
        for name in [
            "sign_editor_plain_oak",
            "sign_editor_hanging_oak",
            "sign_editor_plain_pale_oak",
            "sign_editor_hanging_bamboo",
            "inventory",
            "container_generic",
            "container_crafter_3x3",
            "container_hopper",
            "container_shulker",
        ] {
            let sprite = atlas.sprites[name];
            assert!(sprite.uv.iter().all(|u| (0. ..=1.).contains(u)));
        }
        assert!(atlas.glyphs.contains_key(&'Я'));
        assert!(atlas.glyphs.contains_key(&'A'));
        assert_eq!(atlas.atlas.dimensions(), (2048, 2048));
    }
}
