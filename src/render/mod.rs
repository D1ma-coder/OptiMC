mod camera_motion;
mod chat_editor;
mod draw_list;
mod effects;
mod gui_scale;
mod held;
mod item_highlight;
mod model_scene;
pub use model_scene::Preview;
mod pacing;
mod presentation;
mod profile;
mod remote;
pub mod ui;
use crate::{
    assets,
    player::{DemoCollision, Input, Perspective, Player},
    settings::{Quality, Settings},
    world::{
        lod::{self, Streamer},
        Mesh,
    },
};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec3};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, PhysicalKey},
    window::{CursorGrabMode, CursorIcon, CustomCursor, Window, WindowId},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    matrix: [[f32; 4]; 4],
    inverse_matrix: [[f32; 4]; 4],
    light_matrix: [[f32; 4]; 4],
    eye: [f32; 4],
    settings: [f32; 4],
    fx: [f32; 4],
    sun: [f32; 4],
    relative_matrix: [[f32; 4]; 4],
}
#[derive(Clone)]
struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    count: u32,
    lod_ranges: Arc<[std::ops::Range<u32>]>,
    selected_lod: usize,
    min: Vec3,
    max: Vec3,
    source: Arc<Mesh>,
}
struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    low_pipeline: wgpu::RenderPipeline,
    avatar_pipeline: wgpu::RenderPipeline,
    low_avatar_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    low_translucent_pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    depth: wgpu::TextureView,
    scene_size: [u32; 2],
    meshes: Vec<GpuMesh>,
    draw_list: draw_list::DrawList,
    mesh_lookup: HashMap<usize, usize>,
    visibility: Option<Arc<crate::mods::sodium::Graph>>,
    pending_visibility: Option<Arc<crate::mods::sodium::Graph>>,
    visibility_cache: crate::mods::sodium::Cache,
    avatar: Option<GpuMesh>,
    held: held::Renderer,
    avatar_skin: Option<Arc<image::RgbaImage>>,
    default_skin: Arc<image::RgbaImage>,
    avatar_layers: Option<(
        Arc<image::RgbaImage>,
        crate::mods::skin_layers::Config,
        bool,
        crate::mods::skin_layers::Layers,
    )>,
    pending: Option<(std::vec::IntoIter<Arc<Mesh>>, Vec<GpuMesh>)>,
    adapter: String,
    layout: wgpu::BindGroupLayout,
    terrain_view: wgpu::TextureView,
    terrain_texture: wgpu::Texture,
    uploaded_layers: u32,
    texture_upload: Option<Arc<Vec<u8>>>,
    uploaded_texture_versions: Vec<u64>,
    pending_texture_versions: Vec<u64>,
    texture_cursor: u32,
    terrain_sampler: wgpu::Sampler,
    shadow: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    panorama: wgpu::TextureView,
    panorama_sampler: wgpu::Sampler,
    panorama_valid: bool,
    quality: Quality,
    present_modes: Vec<wgpu::PresentMode>,
    effects: effects::Effects,
    ui: ui::Ui,
    start: Instant,
    profile: Option<profile::Profile>,
}
fn instance() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    })
}
pub fn probe() -> Result<()> {
    let instance = instance();
    let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
    anyhow::ensure!(!adapters.is_empty(),"No Vulkan adapter. Install a Vulkan driver; macOS requires Vulkan Loader + MoltenVK (see README).");
    for a in adapters {
        println!("{:?}", a.get_info());
    }
    Ok(())
}
impl Gpu {
    async fn new(
        window: Arc<Window>,
        cache: &std::path::Path,
        settings: &Settings,
        scene: Option<&model_scene::ModelScene>,
    ) -> Result<Self> {
        assets::ensure_gui(cache)?;
        let mut pixels = assets::texture_pixels(cache)?;
        if let Some(scene) = scene {
            pixels.extend_from_slice(&scene.texture_pixels);
        }
        let layers = (pixels.len() / 16384) as u32;
        let instance = instance();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No Vulkan surface adapter. On macOS install MoltenVK and a Vulkan loader.")?;
        let info = adapter.get_info();
        let texture_capacity = adapter
            .limits()
            .max_texture_array_layers
            .min(2048)
            .max(layers);
        anyhow::ensure!(
            info.backend == wgpu::Backend::Vulkan,
            "Expected Vulkan backend"
        );
        anyhow::ensure!(
            layers <= adapter.limits().max_texture_array_layers,
            "Server scene needs {layers} texture layers; GPU supports {}",
            adapter.limits().max_texture_array_layers
        );
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("OptiMC Vulkan"),
                    required_features: if std::env::var_os("OPTIMC_PROFILE").is_some() {
                        adapter.features() & wgpu::Features::TIMESTAMP_QUERY
                    } else {
                        wgpu::Features::empty()
                    },
                    required_limits: wgpu::Limits {
                        max_texture_array_layers: texture_capacity
                            .max(wgpu::Limits::default().max_texture_array_layers),
                        ..wgpu::Limits::default()
                    },
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;
        anyhow::ensure!(
            layers <= device.limits().max_texture_array_layers,
            "Model requires too many texture layers"
        );
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = *caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .context("No sRGB surface format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | (caps.usages & wgpu::TextureUsages::COPY_SRC),
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let scene_size = [config.width, config.height];
        let depth = depth(&device, scene_size);
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera"),
            contents: bytemuck::bytes_of(&Uniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Official block texture array"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: texture_capacity,
            },
            mip_level_count: 7,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for level in 0..7 {
            let size = 64 >> level;
            let mut mip = Vec::new();
            for layer in pixels.as_chunks::<16384>().0 {
                let image = image::RgbaImage::from_raw(64, 64, layer.to_vec())
                    .expect("validated texture size");
                mip.extend_from_slice(assets::mipmaps::level(&image, size).as_raw());
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mip,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: layers,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Pixel nearest / repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow = effects::shadow(&device, settings.quality);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PCF shadow sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (panorama_pixels, panorama_valid) = assets::panorama_pixels(cache)?;
        let panorama_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Official menu panorama"),
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 6,
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
                texture: &panorama_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &panorama_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512 * 4),
                rows_per_image: Some(512),
            },
            wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 6,
            },
        );
        let panorama = panorama_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let panorama_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Terrain bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&panorama),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&panorama_sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = effects::shader(&device, "Terrain + lighting", include_str!("terrain.wgsl"));
        let terrain_pipeline =
            |target_format, fragment_entry, translucent: bool, depth_write: bool| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Opaque HDR terrain"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[effects::vertex_layout()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fragment_entry),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: target_format,
                            blend: translucent.then_some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: depth_write,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                })
            };
        let pipeline = terrain_pipeline(effects::HDR, "fs_main", false, true);
        let low_pipeline = terrain_pipeline(format, "fs_low", false, true);
        let translucent_pipeline = terrain_pipeline(effects::HDR, "fs_main", true, false);
        let low_translucent_pipeline = terrain_pipeline(format, "fs_low", true, false);
        let avatar_pipeline = terrain_pipeline(effects::HDR, "fs_main", true, true);
        let low_avatar_pipeline = terrain_pipeline(format, "fs_low", true, true);
        let effects = effects::Effects::new(
            &device,
            format,
            [config.width, config.height],
            &layout,
            &uniform,
        );
        let mut ui = ui::Ui::new(&device, &queue, format, cache)?;
        ui.display_size = window
            .current_monitor()
            .map(|m| [m.size().width, m.size().height])
            .unwrap_or([config.width, config.height]);
        if let Some(error) = device.pop_error_scope().await {
            anyhow::bail!("Vulkan pipeline validation: {error}");
        }
        let profile =
            std::env::var_os("OPTIMC_PROFILE").map(|_| profile::Profile::new(&device, &queue));
        let held = held::Renderer::new(&device, cache)?;
        Ok(Self {
            held,
            profile,
            surface,
            device,
            queue,
            config,
            pipeline,
            low_pipeline,
            avatar_pipeline,
            low_avatar_pipeline,
            translucent_pipeline,
            low_translucent_pipeline,
            bind,
            uniform,
            depth,
            scene_size,
            meshes: vec![],
            draw_list: Default::default(),
            mesh_lookup: Default::default(),
            visibility: None,
            pending_visibility: None,
            visibility_cache: Default::default(),
            avatar: None,
            avatar_skin: None,
            default_skin: Arc::new(
                image::open(
                    assets::root(cache)
                        .join("resources/assets/minecraft/textures/entity/player/wide/steve.png"),
                )?
                .to_rgba8(),
            ),
            avatar_layers: None,
            pending: None,
            adapter: info.name,
            layout,
            terrain_view: view,
            terrain_texture: texture,
            uploaded_layers: layers,
            texture_upload: None,
            uploaded_texture_versions: Vec::new(),
            pending_texture_versions: Vec::new(),
            texture_cursor: 7,
            terrain_sampler: sampler,
            shadow,
            shadow_sampler,
            panorama,
            panorama_sampler,
            panorama_valid,
            quality: settings.quality,
            present_modes: caps.present_modes,
            effects,
            ui,
            start: Instant::now(),
        })
    }
    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.scene_size = [size.width, size.height];
        self.depth = depth(&self.device, self.scene_size);
        self.effects
            .resize(&self.device, [size.width, size.height], &self.uniform);
    }
    fn apply_settings(&mut self, settings: &Settings) {
        let mode = presentation::select(settings.vsync, &self.present_modes);
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
            println!("Presentation mode: {mode:?}");
        }
        self.ui.present_mode = mode;
        if self.quality != settings.quality {
            self.quality = settings.quality;
            self.shadow = effects::shadow(&self.device, self.quality);
            self.bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Updated scene bindings"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.terrain_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.terrain_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.shadow),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.panorama),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&self.panorama_sampler),
                    },
                ],
            });
            println!(
                "Graphics: {} / shadow map {}",
                self.quality.label(),
                self.quality.shadow_size()
            );
        }
    }
    fn upload_avatar_skin(&mut self) {
        let pixels = self
            .ui
            .accounts
            .skin
            .as_ref()
            .map_or(&self.default_skin, |skin| &skin.pixels);
        if self
            .avatar_skin
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, pixels))
        {
            return;
        }
        for level in 0..7 {
            let size = 64 >> level;
            let mip = assets::mipmaps::level(pixels, size);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.terrain_texture,
                    mip_level: level,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 6 },
                    aspect: wgpu::TextureAspect::All,
                },
                mip.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.avatar_skin = Some(pixels.clone());
    }
    fn upload(&mut self) {
        let start = Instant::now();
        if let Some(pixels) = &self.texture_upload {
            let end = 7 + (pixels.len() / 16384) as u32;
            while self.texture_cursor < end && start.elapsed() < Duration::from_millis(3) {
                let layer = self.texture_cursor;
                self.texture_cursor += 1;
                let index = layer as usize - 7;
                if self.uploaded_texture_versions.get(index)
                    == self.pending_texture_versions.get(index)
                {
                    continue;
                }
                let offset = (layer - 7) as usize * 16384;
                let image =
                    image::RgbaImage::from_raw(64, 64, pixels[offset..offset + 16384].to_vec())
                        .unwrap();
                let mip_levels = if self.pending_texture_versions[index]
                    & crate::world::remote_mesh::MAP_TILE_VERSION_FLAG
                    != 0
                {
                    1
                } else {
                    7
                };
                for level in 0..mip_levels {
                    let size = 64 >> level;
                    let mip = assets::mipmaps::level(&image, size);
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.terrain_texture,
                            mip_level: level,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        mip.as_raw(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(size * 4),
                            rows_per_image: Some(size),
                        },
                        wgpu::Extent3d {
                            width: size,
                            height: size,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                self.uploaded_layers = self.uploaded_layers.max(layer + 1);
                self.uploaded_texture_versions
                    .resize(self.pending_texture_versions.len(), 0);
                self.uploaded_texture_versions[index] = self.pending_texture_versions[index];
            }
            if self.texture_cursor < end {
                return;
            }
            self.texture_upload = None;
        }
        if let Some((source, target)) = &mut self.pending {
            while start.elapsed() < Duration::from_millis(3) {
                let Some(mesh) = source.next() else {
                    self.meshes = std::mem::take(target);
                    self.mesh_lookup.clear();
                    self.mesh_lookup.extend(
                        self.meshes
                            .iter()
                            .enumerate()
                            .map(|(index, mesh)| (Arc::as_ptr(&mesh.source) as usize, index)),
                    );

                    self.visibility = self.pending_visibility.take();
                    self.pending = None;
                    break;
                };
                if mesh.indices.is_empty() {
                    continue;
                }
                if let Some(existing) = self
                    .mesh_lookup
                    .get(&(Arc::as_ptr(&mesh) as usize))
                    .and_then(|&index| self.meshes.get(index))
                    .filter(|existing| Arc::ptr_eq(&existing.source, &mesh))
                {
                    target.push(existing.clone());
                    continue;
                }
                let mut vertices = mesh.vertices.clone();
                let mut indices = mesh.indices.clone();
                let mut lod_ranges = Vec::with_capacity(5);
                lod_ranges.push(0..indices.len() as u32);
                for variant in &mesh.polygon_lods {
                    if variant.indices.is_empty() {
                        lod_ranges.push(lod_ranges.last().unwrap().clone());
                        continue;
                    }
                    let start = indices.len() as u32;
                    let base = vertices.len() as u32;
                    vertices.extend_from_slice(&variant.vertices);
                    indices.extend(variant.indices.iter().map(|i| i + base));
                    lod_ranges.push(start..indices.len() as u32);
                }
                let vertex = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Terrain vertices and polygon LODs"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let index = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Terrain indices and polygon LODs"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                target.push(GpuMesh {
                    vertex,
                    index,
                    count: mesh.indices.len() as u32,
                    lod_ranges: lod_ranges.into(),
                    selected_lod: 0,
                    min: mesh.min,
                    max: mesh.max,
                    source: mesh,
                });
            }
        }
    }
    fn draw(
        &mut self,
        camera: &Camera,
        radius: i32,
        debug: bool,
        capture: Option<&std::path::Path>,
        settings: &Settings,
        player: Option<&Player>,
    ) -> Result<(usize, u64)> {
        self.apply_settings(settings);
        // Panorama is a separate camera at the origin. Server poses, F5 camera
        // overrides and long-running world coordinates must never freeze it.
        let panorama = Camera::panorama(self.start.elapsed());
        let camera = if !self.ui.in_world && self.panorama_valid {
            &panorama
        } else {
            camera
        };
        let player = player.filter(|_| self.ui.in_world);

        let wanted = if self.ui.in_world {
            settings.render_size(
                [self.config.width, self.config.height],
                self.ui.display_size,
                self.device.limits().max_texture_dimension_2d,
            )
        } else {
            [self.config.width, self.config.height]
        };
        if self.scene_size != wanted {
            self.scene_size = wanted;
            self.depth = depth(&self.device, wanted);
            self.effects.resize(&self.device, wanted, &self.uniform);
            println!(
                "Scene resolution: {}×{}; output: {}×{}",
                wanted[0], wanted[1], self.config.width, self.config.height
            );
        }
        self.upload_avatar_skin();
        self.upload();
        let slim = self.ui.accounts.skin.as_ref().is_some_and(|s| s.slim);
        if settings.skin_layers.enabled {
            let pixels = self.avatar_skin.as_ref().unwrap_or(&self.default_skin);
            if self
                .avatar_layers
                .as_ref()
                .is_none_or(|(old, config, model, _)| {
                    !Arc::ptr_eq(old, pixels) || config != &settings.skin_layers || *model != slim
                })
            {
                self.avatar_layers = Some((
                    pixels.clone(),
                    settings.skin_layers.clone(),
                    slim,
                    crate::mods::skin_layers::Layers::build(pixels, slim, &settings.skin_layers),
                ));
            }
        }
        let layers = self
            .avatar_layers
            .as_ref()
            .filter(|_| {
                settings.skin_layers.enabled
                    && player.is_some_and(|p| {
                        camera.eye.distance(p.eye()) <= settings.skin_layers.render_distance_lod
                    })
            })
            .map(|(_, _, _, layers)| layers);

        let first_person = player
            .is_some_and(|p| p.perspective == Perspective::First && !p.no_clip)
            && !self.ui.hide_hud;
        if first_person {
            let keys = if let Some(store) = &self.ui.server_inventory {
                [
                    store
                        .main_hand(self.ui.inventory.selected as u8)
                        .map(|s| crate::assets::held_items::Key::stack(s, false)),
                    store
                        .player
                        .get(45)
                        .filter(|s| s.count > 0)
                        .map(|s| crate::assets::held_items::Key::stack(s, true)),
                ]
            } else {
                [None, None]
            };
            self.held.prepare(
                held::Resources {
                    device: &self.device,
                    queue: &self.queue,
                    layout: &self.layout,
                    sampler: &self.terrain_sampler,
                    shadow: &self.shadow,
                    shadow_sampler: &self.shadow_sampler,
                    panorama: &self.panorama,
                    panorama_sampler: &self.panorama_sampler,
                },
                keys,
                self.scene_size,
                self.quality,
                camera.bob,
            );
        } else {
            self.held.reset();
        }
        if let Some(player) = player.filter(|p| {
            p.perspective != Perspective::First || (first_person && !self.held.visible[0])
        }) {
            let mesh = if player.perspective == Perspective::First {
                {
                    let direction = camera.direction();
                    crate::world::avatar::hand_at_layers_animated(
                        camera.eye,
                        direction.x.atan2(-direction.z),
                        direction.y.clamp(-1., 1.).asin(),
                        slim,
                        layers.and_then(|l| l.hand.as_ref()),
                        self.held.empty_arm_motion(),
                    )
                }
            } else {
                crate::world::avatar::body_model_layers(
                    player,
                    camera.yaw,
                    camera.pitch,
                    slim,
                    layers,
                )
            };
            if let Some(avatar) = self.avatar.as_mut().filter(|a| {
                a.count == mesh.indices.len() as u32
                    && a.source.vertices.len() == mesh.vertices.len()
            }) {
                self.queue
                    .write_buffer(&avatar.vertex, 0, bytemuck::cast_slice(&mesh.vertices));
            } else {
                let vertex = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Player skin vertices"),
                        contents: bytemuck::cast_slice(&mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                let index = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Player skin indices"),
                        contents: bytemuck::cast_slice(&mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                self.avatar = Some(GpuMesh {
                    vertex,
                    index,
                    count: mesh.indices.len() as u32,
                    min: mesh.min,
                    max: mesh.max,
                    lod_ranges: Arc::from([]),
                    selected_lod: 0,
                    source: Arc::new(mesh),
                });
            }
        } else {
            self.avatar = None;
        }
        let direct = self.scene_size == [self.config.width, self.config.height]
            && self.quality == Quality::Low
            && self.ui.in_world
            && std::env::var_os("OPTIMC_FORCE_HDR").is_none();
        let matrix = camera.matrix(self.scene_size[0] as f32 / self.scene_size[1] as f32);
        let sun = Vec3::new(0.45, 0.78, -0.38).normalize();
        let extent = 192.;
        let texel = extent * 2. / self.quality.shadow_size() as f32;
        let target = Vec3::new(
            (camera.eye.x / texel).floor() * texel,
            36.,
            (camera.eye.z / texel).floor() * texel,
        );
        let light_matrix = Mat4::orthographic_rh(-extent, extent, -extent, extent, 1., 800.)
            * Mat4::look_at_rh(target + sun * 400., target, Vec3::Y);
        let lod_distance = settings.lod_view_distance(
            self.meshes.iter().map(|m| u64::from(m.count / 3)).sum(),
            u64::from(self.scene_size[0]) * u64::from(self.scene_size[1]),
            camera.fov,
        );
        for mesh in &mut self.meshes {
            mesh.selected_lod = crate::world::detail::select(
                crate::world::detail::distance(camera.eye, mesh.min, mesh.max),
                lod_distance,
                mesh.selected_lod,
            )
            .min(mesh.lod_ranges.len().saturating_sub(1));
        }
        let uniform = Uniform {
            relative_matrix: (Mat4::perspective_rh(
                camera.fov.to_radians(),
                self.scene_size[0] as f32 / self.scene_size[1] as f32,
                0.05,
                16384.,
            ) * camera.bob
                * Mat4::look_to_rh(Vec3::ZERO, camera.direction(), Vec3::Y))
            .to_cols_array_2d(),
            matrix: matrix.to_cols_array_2d(),
            inverse_matrix: matrix.inverse().to_cols_array_2d(),
            light_matrix: light_matrix.to_cols_array_2d(),
            eye: [camera.eye.x, camera.eye.y, camera.eye.z, 1.],
            settings: [
                radius as f32 * 0.65,
                radius as f32 * 0.98,
                if debug { 1. } else { 0. },
                0.,
            ],
            fx: [
                self.start.elapsed().as_secs_f32(),
                self.quality.level() as f32,
                if self.ui.in_world { 0. } else { 1. },
                if self.ui.in_world { lod_distance } else { 0. },
            ],
            sun: [
                sun.x,
                sun.y,
                sun.z,
                if self.panorama_valid { 1. } else { 0. },
            ],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        if let Some(profile) = &mut self.profile {
            profile.poll(&self.device);
        }
        let acquire_start = profile::now();
        let frame = self.surface.get_current_texture()?;
        let acquired = profile::now();
        let view = frame.texture.create_view(&Default::default());
        let visible_sections = self.visibility_cache.update(
            if settings.section_occlusion && self.ui.in_world {
                self.visibility.as_ref()
            } else {
                None
            },
            camera.eye.to_array(),
        );
        self.draw_list.update(
            self.meshes
                .iter()
                .map(|m| (m.source.translucent, m.min, m.max, m.source.section)),
            &lod::Frustum::new(matrix),
            visible_sections.as_deref(),
            camera.eye,
            self.ui.in_world || !self.panorama_valid,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });
        let mut draws = 0;
        let mut triangles = 0;
        if self.quality != Quality::Low && self.ui.in_world {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sun shadow map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.effects.shadow_pipeline);
            pass.set_bind_group(0, &self.effects.shadow_bind, &[]);
            let frustum = lod::Frustum::new(light_matrix);
            for mesh in &self.meshes {
                if mesh.source.translucent || !frustum.visible(mesh.min, mesh.max) {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.lod_ranges[mesh.selected_lod].clone(), 0, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Terrain pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if direct { &view } else { &self.effects.hdr },
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.48,
                            g: 0.69,
                            b: 0.88,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.profile.as_ref().and_then(|p| p.timestamps()),
                occlusion_query_set: None,
            });
            pass.set_pipeline(if direct {
                &self.effects.low_sky
            } else {
                &self.effects.sky
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(if direct {
                &self.low_pipeline
            } else {
                &self.pipeline
            });
            pass.set_bind_group(0, &self.bind, &[]);
            for &index in &self.draw_list.opaque {
                let mesh = &self.meshes[index];
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.lod_ranges[mesh.selected_lod].clone(), 0, 0..1);
                draws += 1;
                triangles += u64::from(mesh.lod_ranges[mesh.selected_lod].len() as u32 / 3);
            }
            pass.set_pipeline(if direct {
                &self.low_translucent_pipeline
            } else {
                &self.translucent_pipeline
            });
            for &(index, _) in &self.draw_list.transparent {
                let mesh = &self.meshes[index];
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.lod_ranges[mesh.selected_lod].clone(), 0, 0..1);
                draws += 1;
                triangles += u64::from(mesh.lod_ranges[mesh.selected_lod].len() as u32 / 3);
            }
        }
        if let Some(avatar) = &self.avatar {
            let first = player.is_some_and(|p| p.perspective == Perspective::First);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Player / first-person arm"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if direct { &view } else { &self.effects.hdr },
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if first {
                            wgpu::LoadOp::Clear(1.)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(if direct {
                &self.low_avatar_pipeline
            } else {
                &self.avatar_pipeline
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, avatar.vertex.slice(..));
            pass.set_index_buffer(avatar.index.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..avatar.count, 0, 0..1);
        }
        if first_person {
            self.held.draw(
                &mut encoder,
                if direct { &view } else { &self.effects.hdr },
                &self.depth,
                if direct {
                    &self.low_avatar_pipeline
                } else {
                    &self.avatar_pipeline
                },
            );
        }
        if !direct {
            self.effects.post(&mut encoder, &view);
        }
        self.ui.draw(&mut encoder, &view);
        let readback = if capture.is_some() {
            anyhow::ensure!(
                self.config.usage.contains(wgpu::TextureUsages::COPY_SRC),
                "Surface cannot be captured on this adapter"
            );
            let stride = (self.config.width * 4).div_ceil(256) * 256;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Screenshot readback"),
                size: stride as u64 * self.config.height as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(self.config.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
            );
            Some((buffer, stride))
        } else {
            None
        };
        let sampled = self
            .profile
            .as_ref()
            .is_some_and(|p| p.resolve(&mut encoder));
        let encoded = profile::now();
        self.queue.submit(Some(encoder.finish()));
        let submitted = profile::now();
        if let Some(profile) = &mut self.profile {
            profile.submitted(sampled);
        }
        if let (Some(path), Some((buffer, stride))) = (capture, readback) {
            let (send, recv) = mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = send.send(result);
                });
            self.device.poll(wgpu::Maintain::Wait);
            recv.recv()??;
            let mapped = buffer.slice(..).get_mapped_range();
            let mut pixels =
                Vec::with_capacity((self.config.width * self.config.height * 4) as usize);
            for row in mapped.chunks(stride as usize) {
                pixels.extend_from_slice(&row[..self.config.width as usize * 4]);
            }
            match self.config.format {
                wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm => {
                    for pixel in pixels.chunks_mut(4) {
                        pixel.swap(0, 2);
                    }
                }
                wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => {}
                format => anyhow::bail!("Unsupported capture format {format:?}"),
            }
            image::save_buffer(
                path,
                &pixels,
                self.config.width,
                self.config.height,
                image::ColorType::Rgba8,
            )?;
            drop(mapped);
            buffer.unmap();
            println!("Vulkan screenshot: {}", path.display());
        }
        let present_start = profile::now();
        frame.present();
        if let Some(profile) = &mut self.profile {
            profile.frame(
                acquired - acquire_start,
                encoded - acquired,
                submitted - encoded,
                present_start.elapsed(),
            );
        }
        Ok((draws, triangles))
    }
}
fn depth(device: &wgpu::Device, size: [u32; 2]) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
#[derive(Clone)]
struct Camera {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
    speed: f32,
    direction_override: Option<Vec3>,
    bob: Mat4,
    fov: f32,
}
impl Camera {
    fn panorama(elapsed: Duration) -> Self {
        Self {
            eye: Vec3::ZERO,
            yaw: ((elapsed.as_secs_f64() * 2.).rem_euclid(360.) as f32).to_radians(),
            pitch: -10f32.to_radians(),
            speed: 0.,
            direction_override: None,
            bob: Mat4::IDENTITY,
            fov: 85.,
        }
    }

    fn direction(&self) -> Vec3 {
        self.direction_override.unwrap_or_else(|| {
            Vec3::new(
                self.yaw.sin() * self.pitch.cos(),
                self.pitch.sin(),
                -self.yaw.cos() * self.pitch.cos(),
            )
        })
    }
    fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), aspect, 0.05, 16384.)
            * self.bob
            * Mat4::look_at_rh(self.eye, self.eye + self.direction(), Vec3::Y)
    }
    fn update(&mut self, keys: &HashSet<KeyCode>, dt: f32) {
        let f = Vec3::new(self.yaw.sin(), 0., -self.yaw.cos());
        let r = f.cross(Vec3::Y);
        let mut v = Vec3::ZERO;
        for (key, delta) in [
            (KeyCode::KeyW, f),
            (KeyCode::KeyS, -f),
            (KeyCode::KeyD, r),
            (KeyCode::KeyA, -r),
            (KeyCode::Space, Vec3::Y),
            (KeyCode::ShiftLeft, Vec3::NEG_Y),
        ] {
            if keys.contains(&key) {
                v += delta;
            }
        }
        self.eye += v.normalize_or_zero()
            * self.speed
            * dt
            * if keys.contains(&KeyCode::ControlLeft) {
                4.
            } else {
                1.
            };
    }
}
struct App {
    cache: PathBuf,
    radius: i32,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    camera: Camera,
    scene: Option<Arc<model_scene::ModelScene>>,
    remote: Option<remote::RemoteSession>,
    remote_frame: Option<remote::Scene>,
    remote_epoch: u64,
    remote_teleport: u64,
    remote_velocity: u64,
    remote_abilities: u64,
    remote_selected_slot: u64,
    remote_visible: bool,
    remote_geometry: Option<Arc<crate::net::live::Snapshot>>,
    launch_server: Option<(crate::net::Address, String)>,
    player: Option<Player>,
    keys: HashSet<KeyCode>,
    captured: bool,
    attack_held: bool,
    mining: crate::world::mining::Mining,
    use_held: bool,
    use_delay: u8,
    hidden_cursor: Option<CustomCursor>,
    debug: bool,
    request: SyncSender<Vec3>,
    response: Receiver<Vec<Arc<Mesh>>>,
    last_tile: (i32, i32),
    last_frame: Instant,
    title_time: Instant,
    frames: u32,
    error: Option<anyhow::Error>,
    screenshot: Option<PathBuf>,
    settings: Settings,
    start_screen: ui::Screen,
    frame_pacer: pacing::Pacer,
    hotbar_wheel: crate::scroll::Wheel,
    zoom: crate::mods::zoom::Controller,
    camera_motion: camera_motion::Motion,
    frame_limit: Option<u32>,
    measured_frames: u32,
    measurement_start: Option<Instant>,
}
impl App {
    fn input_bits(&self) -> u8 {
        let down = |key| self.keys.contains(&key);
        down(KeyCode::KeyW) as u8
            | ((down(KeyCode::KeyS) as u8) << 1)
            | ((down(KeyCode::KeyA) as u8) << 2)
            | ((down(KeyCode::KeyD) as u8) << 3)
            | ((down(KeyCode::Space) as u8) << 4)
            | (((down(KeyCode::ShiftLeft) || down(KeyCode::ShiftRight)) as u8) << 5)
            | (((down(KeyCode::ControlLeft) || down(KeyCode::ControlRight)) as u8) << 6)
    }
    fn inventory_action(&self, action: crate::net::request::InventoryAction) -> Result<()> {
        let (Some(remote), Some(gpu)) = (&self.remote, &self.gpu) else {
            return Ok(());
        };
        let Some(snapshot) = &remote.snapshot else {
            return Ok(());
        };
        if !self.captured
            || gpu.ui.screen != ui::Screen::Playing
            || !snapshot.client_loaded
            || !snapshot.position_ready
            || snapshot
                .report
                .world
                .spawn
                .as_ref()
                .is_some_and(|s| s.game_mode == 3)
        {
            return Ok(());
        }
        remote.request(crate::net::request::Request::InventoryAction {
            action,
            slot: gpu.ui.inventory.selected as u8,
            input: self.input_bits(),
            teleport_version: snapshot.teleport_version,
        })
    }
    fn stop_mining(&mut self) -> Result<()> {
        if let Some((action, hit)) = self.mining.abort() {
            if let (Some(remote), Some(gpu)) = (&self.remote, &self.gpu) {
                if let Some(snapshot) = &remote.snapshot {
                    remote.request(crate::net::request::Request::Dig {
                        action,
                        block: hit.block,
                        face: hit.face,
                        slot: gpu.ui.inventory.selected as u8,
                        input: self.input_bits(),
                        teleport_version: snapshot.teleport_version,
                    })?;
                }
            }
        }
        Ok(())
    }
    fn interact_world(&mut self, attack: bool) -> Result<()> {
        self.interact_world_inner(attack, false)
    }
    fn interact_world_inner(&mut self, attack: bool, continuing: bool) -> Result<()> {
        let (Some(remote), Some(player), Some(gpu)) = (&self.remote, &self.player, &self.gpu)
        else {
            return Ok(());
        };
        let Some(snapshot) = &remote.snapshot else {
            return Ok(());
        };
        if !snapshot.client_loaded
            || !snapshot.position_ready
            || gpu.ui.screen != ui::Screen::Playing
            || !self.captured
        {
            return Ok(());
        }
        let world = &snapshot.report.world;
        // Spectator container inspection needs a distinct client-side gate.
        if world
            .spawn
            .as_ref()
            .is_some_and(|spawn| spawn.game_mode == 3)
        {
            return Ok(());
        }
        let reach = world
            .player_id
            .and_then(|id| {
                snapshot
                    .report
                    .attributes
                    .value(id, "minecraft:block_interaction_range")
            })
            .unwrap_or(4.5);
        let (yaw, pitch) = (f64::from(self.camera.yaw), f64::from(self.camera.pitch));
        let direction = glam::DVec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        );
        let entity_reach = world
            .player_id
            .and_then(|id| {
                snapshot
                    .report
                    .attributes
                    .value(id, "minecraft:entity_interaction_range")
            })
            .unwrap_or(3.);
        let target = crate::world::entity_pick::target(
            &snapshot.report,
            player.eye_precise(),
            direction,
            reach,
            entity_reach,
        );
        let input = self.input_bits();
        use crate::{net::request::Request, world::entity_pick::Target};
        let hand = crate::net::entity::Hand::Main;
        let slot = gpu.ui.inventory.selected as u8;
        let teleport_version = snapshot.teleport_version;
        if attack && (!continuing || self.mining.is_active()) {
            if let Some(gpu) = &mut self.gpu {
                gpu.held.swing();
            }
        }
        self.mining
            .generation(snapshot.world_epoch, snapshot.teleport_version);
        if attack {
            if let Some(Target::Block(hit)) = target {
                let game_mode = world.spawn.as_ref().map_or(0, |s| s.game_mode);
                if game_mode == 0 || game_mode == 1 {
                    if let Some(state) = world.block(hit.block[0], hit.block[1], hit.block[2]) {
                        let delta = crate::world::mining::progress(
                            &snapshot.report,
                            state,
                            slot,
                            player.grounded,
                            player.eye_precise(),
                        );
                        let hand = snapshot
                            .report
                            .inventory
                            .player
                            .get(36 + slot as usize)
                            .filter(|s| s.count > 0)
                            .cloned();
                        if player.abilities.instant_build
                            && hand
                                .as_ref()
                                .and_then(|s| s.tool())
                                .is_some_and(|t| !t.creative)
                        {
                            if let Some((action, old)) = self.mining.abort() {
                                remote.request(Request::Dig {
                                    action,
                                    block: old.block,
                                    face: old.face,
                                    slot,
                                    input,
                                    teleport_version,
                                })?;
                            }
                            if !continuing {
                                remote.request(Request::Swing {
                                    slot,
                                    input,
                                    teleport_version,
                                })?;
                            }
                            return Ok(());
                        }
                        let actions = self.mining.update(
                            hit,
                            state,
                            hand,
                            delta,
                            player.abilities.instant_build,
                            continuing,
                        );
                        let send_swing = !actions.is_empty() || self.mining.is_active();
                        for (action, hit) in actions {
                            remote.request(Request::Dig {
                                action,
                                block: hit.block,
                                face: hit.face,
                                slot,
                                input,
                                teleport_version,
                            })?;
                        }
                        if send_swing {
                            remote.request(Request::Swing {
                                slot,
                                input,
                                teleport_version,
                            })?;
                        }
                        return Ok(());
                    }
                }
            }
            if let Some((action, hit)) = self.mining.abort() {
                remote.request(Request::Dig {
                    action,
                    block: hit.block,
                    face: hit.face,
                    slot,
                    input,
                    teleport_version,
                })?;
            }
            if continuing {
                return Ok(());
            }
        }
        let request = match target {
            Some(Target::Entity(hit)) => Request::EntityAction {
                hit,
                attack,
                hand,
                slot,
                input,
                teleport_version,
            },
            Some(Target::Block(hit)) if !attack => Request::UseBlock {
                hit,
                hand,
                slot,
                input,
                teleport_version,
            },
            _ if attack => Request::Swing {
                slot,
                input,
                teleport_version,
            },
            _ => Request::UseItem {
                hand: if snapshot.report.inventory.main_hand(slot).is_some() {
                    hand
                } else if snapshot
                    .report
                    .inventory
                    .player
                    .get(45)
                    .is_some_and(|s| s.count > 0)
                {
                    crate::net::entity::Hand::Off
                } else {
                    return Ok(());
                },
                slot,
                input,
                yaw: self.camera.yaw.to_degrees() - 180.,
                pitch: -self.camera.pitch.to_degrees(),
                teleport_version,
            },
        };
        if !attack {
            let left = matches!(
                &request,
                Request::UseItem {
                    hand: crate::net::entity::Hand::Off,
                    ..
                }
            );
            if let Some(gpu) = &mut self.gpu {
                gpu.held.swing_hand(left);
            }
        }
        remote.request(request)
    }

    fn begin_remote(&mut self, address: crate::net::Address, username: String) -> Result<()> {
        self.end_remote(ui::Screen::Connecting);
        self.remote = Some(remote::RemoteSession::new(
            self.cache.clone(),
            address,
            username.clone(),
        )?);
        self.remote_epoch = 0;
        self.remote_teleport = 0;
        self.remote_velocity = u64::MAX;
        self.remote_abilities = u64::MAX;
        self.remote_selected_slot = u64::MAX;
        self.scene = None;
        self.player = None;
        self.camera.direction_override = None;
        self.camera_motion.reset();
        self.camera.bob = Mat4::IDENTITY;
        self.camera.fov = 70.;
        if let Some(gpu) = &mut self.gpu {
            gpu.ui.server_inventory = None;
            gpu.ui.reset_sign_editor();
            gpu.held.reset();
            gpu.ui.username = username;
            gpu.uploaded_layers = 7;
            gpu.uploaded_texture_versions.clear();
            gpu.ui.hide_hud = false;
            gpu.ui.inventory = crate::inventory::Inventory {
                slots: [None; 46],
                cursor: None,
                selected: 0,
            };
            gpu.ui.server_scoreboard = Default::default();
            gpu.ui.server_players = Default::default();
            gpu.ui.server_commands = Default::default();
            gpu.ui.message = "Подключение...".into();
            gpu.ui.rich_reason = None;
            gpu.ui.server_messages = Default::default();
        }
        Ok(())
    }
    fn end_remote(&mut self, screen: ui::Screen) {
        self.remote = None;
        self.remote_frame = None;
        self.remote_visible = false;
        self.remote_geometry = None;
        self.player = None;
        self.keys.clear();
        self.capture(false);
        if let Some(gpu) = &mut self.gpu {
            gpu.pending = None;
            gpu.texture_upload = None;
            gpu.uploaded_texture_versions.clear();
            gpu.meshes.clear();
            gpu.mesh_lookup.clear();
            gpu.visibility = None;
            gpu.pending_visibility = None;
            gpu.avatar = None;
            gpu.ui.server_inventory = None;
            gpu.ui.reset_sign_editor();
            gpu.held.reset();
            gpu.ui.server_hud = Default::default();
            gpu.ui.server_scoreboard = Default::default();
            gpu.ui.server_players = Default::default();
            gpu.ui.server_commands = Default::default();
            gpu.ui.server_messages = Default::default();
            gpu.ui.vitals = Default::default();
            gpu.ui.in_world = false;
            gpu.ui.screen = screen;
        }
        while self.response.try_recv().is_ok() {}
        self.last_tile = (i32::MAX, i32::MAX);
    }
    fn poll_remote(&mut self) -> Result<()> {
        use crate::net::live::Status;
        let Some(remote) = &mut self.remote else {
            return Ok(());
        };
        if let Some(scene) = remote.poll() {
            self.remote_frame = Some(scene.map_err(anyhow::Error::msg)?);
        }
        let status = remote.status();
        match status {
            Status::Closed(reason) => anyhow::bail!("{reason}"),
            Status::Rejected(reason) => {
                return Err(crate::net::component::Disconnect(reason).into())
            }
            _ => {}
        }
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        if let Some(snapshot) = &remote.snapshot {
            gpu.ui.server_messages = snapshot.report.messages.clone();
            gpu.ui.vitals = snapshot.report.vitals;
            gpu.ui.server_hud = snapshot.report.hud.clone();
            gpu.ui.server_scoreboard = snapshot.report.scoreboard.clone();
            gpu.ui.server_players = snapshot.report.player_info.clone();
            gpu.ui.server_commands = snapshot.report.commands.clone();
            gpu.ui.sync_inventory(snapshot.report.inventory.clone());
            if let Some((slot, serial)) = snapshot.report.selected_slot {
                if self.remote_selected_slot != serial {
                    gpu.ui.inventory.selected = slot as usize;
                    self.remote_selected_slot = serial;
                }
            }
            gpu.ui.creative_inventory = snapshot.abilities.instant_build;
            gpu.ui.survival_hud = snapshot
                .report
                .world
                .spawn
                .as_ref()
                .is_none_or(|spawn| spawn.game_mode == 0 || spawn.game_mode == 2);
            if snapshot.world_epoch != self.remote_epoch {
                self.player = None;
                self.remote_epoch = snapshot.world_epoch;
                self.remote_visible = false;
                self.remote_geometry = None;
                gpu.meshes.clear();
                gpu.mesh_lookup.clear();
                gpu.visibility = None;
                gpu.pending_visibility = None;
                gpu.pending = None;
                gpu.texture_upload = None;
                gpu.uploaded_texture_versions.clear();
                gpu.ui.in_world = false;
                gpu.ui.screen = ui::Screen::Connecting;
            }
            if snapshot.position_ready && snapshot.teleport_version != self.remote_teleport {
                self.remote_teleport = snapshot.teleport_version;
                self.camera.eye =
                    Vec3::from_array(snapshot.position.map(|v| v as f32)) + Vec3::Y * 1.62;
                self.camera.yaw = (snapshot.rotation[0] + 180.).to_radians();
                self.camera.pitch = -snapshot.rotation[1].to_radians();
                let player = self
                    .player
                    .get_or_insert_with(|| Player::spawn(snapshot.position.into()));
                player.correct(crate::net::entity::Pose {
                    position: snapshot.position.into(),
                    velocity: snapshot.velocity.into(),
                    yaw: snapshot.rotation[0],
                    pitch: snapshot.rotation[1],
                });
                self.remote_abilities = u64::MAX;
            }
            if let Some(player) = &mut self.player {
                player.food = snapshot.report.vitals.food;
                if let Some(id) = snapshot.report.world.player_id {
                    player.attributes = snapshot.report.attributes.movement(id);
                }
                if self.remote_velocity != snapshot.velocity_version {
                    player.velocity = snapshot.velocity.into();
                    self.remote_velocity = snapshot.velocity_version;
                }
                if self.remote_abilities != snapshot.abilities_version {
                    player.abilities = snapshot.abilities;
                    self.remote_abilities = snapshot.abilities_version;
                }
                player.no_clip = snapshot
                    .report
                    .world
                    .spawn
                    .as_ref()
                    .is_some_and(|s| s.game_mode == 3);
            }
            if self
                .remote_frame
                .as_ref()
                .is_some_and(|frame| frame.snapshot.world_epoch != snapshot.world_epoch)
            {
                self.remote_frame = None;
            }
            if gpu.pending.is_none() && gpu.texture_upload.is_none() {
                if let Some(frame) = self.remote_frame.take() {
                    anyhow::ensure!(
                        frame.pixels.len() == frame.texture_versions.len() * 16384,
                        "Server texture/version count mismatch"
                    );
                    anyhow::ensure!(
                        7 + frame.pixels.len() / 16384
                            <= gpu.terrain_texture.depth_or_array_layers() as usize,
                        "GPU texture-array limit exceeded by server world"
                    );
                    if frame.meshes.len() != gpu.meshes.len()
                        || frame.texture_versions.len() != gpu.uploaded_texture_versions.len()
                        || std::env::var_os("OPTIMC_NET_TRACE").is_some()
                    {
                        println!(
                            "Live scene: {} sections/meshes; {} chunk packets; {} textures",
                            frame.meshes.len(),
                            frame.snapshot.report.chunks,
                            frame.pixels.len() / 16384
                        );
                    }
                    gpu.texture_upload = Some(frame.pixels);
                    gpu.pending_texture_versions = frame.texture_versions.as_ref().clone();
                    gpu.texture_cursor = 7;
                    gpu.pending = Some((frame.meshes.into_iter(), vec![]));
                    gpu.pending_visibility = Some(frame.visibility);
                    self.remote_geometry = Some(frame.snapshot);
                    self.remote_visible = true;
                }
            }
            if !snapshot.client_loaded && snapshot.position_ready {
                let p = self.camera.eye.as_dvec3().to_array();
                let compiled = self.remote_visible
                    && gpu.pending.is_none()
                    && gpu.texture_upload.is_none()
                    && self.remote_geometry.as_ref().is_some_and(|geometry| {
                        remote::compiled_camera_column(
                            geometry.world_epoch,
                            snapshot.world_epoch,
                            &geometry.report.world,
                            &snapshot.report.world,
                            p,
                        )
                    });
                remote.load_progress(
                    snapshot.world_epoch,
                    snapshot.teleport_version,
                    compiled,
                    Some(f64::from(self.camera.eye.y)),
                );
            }
            gpu.ui
                .sync_sign_editor(snapshot.world_epoch, &snapshot.report.sign_editor);
            if snapshot.client_loaded && snapshot.position_ready && !gpu.ui.in_world {
                gpu.ui.screen = ui::Screen::Playing;
                gpu.ui.in_world = true;
            }
        }
        if gpu.ui.screen == ui::Screen::Connecting {
            gpu.ui.message = match status {
                Status::Connecting => "Подключение...",
                Status::Login => "Вход на сервер...",
                Status::Configuration => "Получение данных сервера...",
                Status::Playing => "Загрузка мира...",
                _ => "Соединение закрыто",
            }
            .into();
        }
        Ok(())
    }
    fn capture(&mut self, capture: bool) {
        if !capture {
            self.attack_held = false;
            if let Err(error) = self.stop_mining() {
                eprintln!("Stop mining on capture loss: {error:#}");
            }
            self.use_held = false;
            self.use_delay = 0;
            self.zoom.cancel();
        }
        if let Some(window) = &self.window {
            if capture {
                if window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                    .is_err()
                {
                    return;
                }
            } else {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
            }
            // AppKit may reset cursor rectangles during relative mouse motion.
            // A transparent cursor stays invisible even during that reset.
            if capture {
                if let Some(cursor) = &self.hidden_cursor {
                    window.set_cursor(cursor.clone());
                }
            } else {
                window.set_cursor(CursorIcon::Default);
            }
            window.set_cursor_visible(!capture);
            window.set_ime_allowed(!capture);
            self.captured = capture;
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<()> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("OptiMC · Vulkan · terrain laboratory")
                        .with_inner_size(PhysicalSize::new(1440, 900))
                        .with_window_icon(
                            crate::assets::icons::read(&self.cache, "icon_32x32.png")
                                .ok()
                                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                                .and_then(|image| {
                                    let rgba = image.to_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    winit::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
                                }),
                        ),
                )?,
            );
            #[cfg(target_os = "macos")]
            if let Err(error) = crate::assets::icons::apply_macos(&self.cache) {
                eprintln!("Cannot set official application icon: {error:#}");
            }
            let mut gpu = pollster::block_on(Gpu::new(
                window.clone(),
                &self.cache,
                &self.settings,
                self.scene.as_deref(),
            ))?;
            gpu.ui.hide_hud = self.scene.is_some();
            gpu.ui.screen = self.start_screen;
            gpu.ui.in_world = matches!(
                self.start_screen,
                ui::Screen::Playing | ui::Screen::Paused | ui::Screen::Inventory
            );
            println!("Vulkan device: {}", gpu.adapter);
            println!("Click: mouse capture; WASD: move; Space: jump; Shift: sneak; Ctrl: sprint; E: inventory; 1-9 / wheel: hotbar; F: offhand; F5: camera; F1: HUD; Esc: pause");
            self.hidden_cursor = Some(event_loop.create_custom_cursor(CustomCursor::from_rgba(
                vec![0; 16],
                2,
                2,
                0,
                0,
            )?));
            self.gpu = Some(gpu);
            self.window = Some(window);
            if let Some((address, username)) = self.launch_server.take() {
                self.begin_remote(address, username)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(e);
            event_loop.exit();
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
        if self.captured {
            if let DeviceEvent::MouseMotion { delta } = event {
                let sensitivity = (self.settings.mouse_radians()
                    * self.zoom.sensitivity(&self.settings.zoom))
                    as f32;
                self.camera.yaw += delta.0 as f32
                    * sensitivity
                    * if self.settings.invert_x_mouse {
                        -1.
                    } else {
                        1.
                    };
                self.camera.pitch = (self.camera.pitch
                    - delta.1 as f32
                        * sensitivity
                        * if self.settings.invert_y_mouse {
                            -1.
                        } else {
                            1.
                        })
                .clamp(-1.55, 1.55);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(gpu) = &mut self.gpu {
            gpu.ui.event(&event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Moved(_) => {
                if let (Some(window), Some(gpu)) = (&self.window, &mut self.gpu) {
                    if let Some(monitor) = window.current_monitor() {
                        gpu.ui.display_size = [monitor.size().width, monitor.size().height];
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(monitor) = self.window.as_ref().and_then(|w| w.current_monitor()) {
                        gpu.ui.display_size = [monitor.size().width, monitor.size().height];
                    }
                    gpu.resize(size);
                }
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.capture(false);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if !state.is_pressed() {
                    self.attack_held = false;
                    if let Err(error) = self.stop_mining() {
                        eprintln!("Stop mining: {error:#}");
                    }
                } else if self
                    .gpu
                    .as_ref()
                    .is_some_and(|gpu| gpu.ui.screen == ui::Screen::Playing)
                {
                    if self.captured {
                        self.attack_held = true;
                        if let Err(error) = self.interact_world(true) {
                            eprintln!("Attack/mining: {error:#}");
                        }
                    } else {
                        self.capture(true);
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => {
                let playing = self
                    .gpu
                    .as_ref()
                    .is_some_and(|g| g.ui.screen == ui::Screen::Playing);
                self.use_held = state.is_pressed() && playing && self.captured;
                if self.use_held && self.use_delay == 0 {
                    if let Err(error) = self.interact_world(false) {
                        eprintln!("Block use: {error:#}");
                    }
                    self.use_delay = 4;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(gpu) = &mut self.gpu {
                    if gpu.ui.screen == ui::Screen::Playing {
                        let scale = self
                            .window
                            .as_ref()
                            .map_or(1., |window| window.scale_factor());
                        if self.zoom.active(&self.settings.zoom) && self.settings.zoom.scroll_zoom {
                            let steps = self.hotbar_wheel.steps(
                                delta,
                                scale,
                                self.settings.wheel_sensitivity,
                                self.settings.discrete_scroll,
                            );
                            self.zoom
                                .scroll(steps[0].saturating_add(steps[1]), &self.settings.zoom);
                            return;
                        }
                        let step = self.hotbar_wheel.hotbar_settings(
                            delta,
                            scale,
                            self.settings.wheel_sensitivity,
                            self.settings.discrete_scroll,
                        );
                        if step != 0 {
                            gpu.ui.inventory.scroll(step);
                        }
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if self.gpu.as_ref().is_some_and(|g| {
                    matches!(g.ui.screen, ui::Screen::Chat | ui::Screen::SignEditor)
                }) && event.physical_key != PhysicalKey::Code(KeyCode::Escape)
                {
                    return;
                }
                let open_chat = event.state.is_pressed()
                    && !event.repeat
                    && self.remote.is_some()
                    && self
                        .gpu
                        .as_ref()
                        .is_some_and(|g| g.ui.screen == ui::Screen::Playing)
                    && (matches!(
                        event.physical_key,
                        PhysicalKey::Code(KeyCode::KeyT | KeyCode::Slash)
                    ) || matches!(&event.logical_key,Key::Character(c) if c=="/"));
                if open_chat {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.ui.open_chat(
                            event.physical_key == PhysicalKey::Code(KeyCode::Slash)
                                || matches!(&event.logical_key,Key::Character(c) if c=="/"),
                        );
                    }
                    self.keys.clear();
                    self.capture(false);
                    return;
                }
                if event.physical_key == PhysicalKey::Code(KeyCode::KeyC)
                    && self
                        .gpu
                        .as_ref()
                        .is_some_and(|g| g.ui.screen == ui::Screen::Playing)
                {
                    self.zoom.key(event.state.is_pressed(), &self.settings.zoom);
                    return;
                }
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state.is_pressed() {
                        self.keys.insert(key);
                        if !event.repeat {
                            match key {
                                KeyCode::Escape => {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.ui.escape();
                                    }
                                    let playing = self
                                        .gpu
                                        .as_ref()
                                        .is_some_and(|g| g.ui.screen == ui::Screen::Playing);
                                    self.keys.clear();
                                    self.capture(playing);
                                }
                                KeyCode::F11 => {
                                    if let Some(window) = &self.window {
                                        window.set_fullscreen(if window.fullscreen().is_some() {
                                            None
                                        } else {
                                            Some(winit::window::Fullscreen::Borderless(
                                                window.current_monitor(),
                                            ))
                                        });
                                    }
                                }
                                KeyCode::F3 => {
                                    if let Some(gpu) = &mut self.gpu {
                                        if gpu.ui.in_world {
                                            gpu.ui.debug_overlay = !gpu.ui.debug_overlay;
                                        }
                                    }
                                }
                                KeyCode::F5 => {
                                    if let Some(player) = &mut self.player {
                                        player.perspective = player.perspective.cycle();
                                    }
                                }
                                KeyCode::F1 => {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.ui.hide_hud = !gpu.ui.hide_hud;
                                    }
                                }
                                KeyCode::KeyE => {
                                    if let Some(gpu) = &mut self.gpu {
                                        match gpu.ui.screen {
                                            ui::Screen::Playing => {
                                                gpu.ui.screen = ui::Screen::Inventory
                                            }
                                            ui::Screen::Inventory | ui::Screen::Container => {
                                                gpu.ui.escape()
                                            }
                                            _ => {}
                                        }
                                    }
                                    let playing = self
                                        .gpu
                                        .as_ref()
                                        .is_some_and(|g| g.ui.screen == ui::Screen::Playing);
                                    self.keys.clear();
                                    self.capture(playing);
                                }
                                KeyCode::KeyF => {
                                    if self.remote.is_some() {
                                        if let Err(error) = self.inventory_action(
                                            crate::net::request::InventoryAction::SwapHands,
                                        ) {
                                            eprintln!("Swap hands: {error:#}");
                                        }
                                    } else if let Some(gpu) = &mut self.gpu {
                                        if gpu.ui.screen == ui::Screen::Playing {
                                            gpu.ui.inventory.swap_offhand();
                                        }
                                    }
                                }
                                KeyCode::KeyQ => {
                                    let whole = self.keys.contains(&KeyCode::ControlLeft)
                                        || self.keys.contains(&KeyCode::ControlRight);
                                    if self.remote.is_some() {
                                        if let Err(error) = self.inventory_action(if whole {
                                            crate::net::request::InventoryAction::DropAll
                                        } else {
                                            crate::net::request::InventoryAction::DropItem
                                        }) {
                                            eprintln!("Drop item: {error:#}");
                                        }
                                    } else if let Some(gpu) = &mut self.gpu {
                                        if gpu.ui.screen == ui::Screen::Playing {
                                            gpu.ui.inventory.drop_selected(whole);
                                        }
                                    }
                                }
                                KeyCode::Digit1
                                | KeyCode::Digit2
                                | KeyCode::Digit3
                                | KeyCode::Digit4
                                | KeyCode::Digit5
                                | KeyCode::Digit6
                                | KeyCode::Digit7
                                | KeyCode::Digit8
                                | KeyCode::Digit9 => {
                                    let digits = [
                                        KeyCode::Digit1,
                                        KeyCode::Digit2,
                                        KeyCode::Digit3,
                                        KeyCode::Digit4,
                                        KeyCode::Digit5,
                                        KeyCode::Digit6,
                                        KeyCode::Digit7,
                                        KeyCode::Digit8,
                                        KeyCode::Digit9,
                                    ];
                                    if let Some(gpu) = &mut self.gpu {
                                        if gpu.ui.screen == ui::Screen::Playing {
                                            gpu.ui.inventory.selected =
                                                digits.iter().position(|d| *d == key).unwrap();
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        self.keys.remove(&key);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let camera_dt = (now - self.last_frame).as_secs_f32();
                let dt = camera_dt.min(0.1);
                self.last_frame = now;
                let mut actions = Vec::new();
                if let Some(gpu) = &mut self.gpu {
                    if gpu.ui.debug_overlay && gpu.ui.in_world {
                        let feet = self
                            .player
                            .as_ref()
                            .map_or(self.camera.eye.as_dvec3(), Player::interpolated_feet);
                        let block = feet.floor().as_ivec3();
                        let chunk = block.div_euclid(glam::IVec3::splat(16));
                        let yaw = self.camera.yaw.to_degrees() - 180.;
                        let facing = match ((yaw.rem_euclid(360.) + 45.) / 90.).floor() as i32 % 4 {
                            0 => "south (Towards positive Z)",
                            1 => "west (Towards negative X)",
                            2 => "north (Towards negative Z)",
                            _ => "east (Towards positive X)",
                        };
                        gpu.ui.debug_left = vec![
                            "Minecraft 26.2 (OptiMC / independent Rust client)".into(),
                            format!(
                                "{:.0} fps T: {} {}",
                                gpu.ui.fps,
                                if self.settings.fps_limit == 0 {
                                    "inf".into()
                                } else {
                                    self.settings.fps_limit.to_string()
                                },
                                if self.settings.vsync { "vsync" } else { "" }
                            ),
                            format!(
                                "C: {}/{} D: {}",
                                gpu.ui.last_draws,
                                gpu.meshes.len(),
                                self.radius / 16
                            ),
                            format!("XYZ: {:.3} / {:.5} / {:.3}", feet.x, feet.y, feet.z),
                            format!("Block: {} {} {}", block.x, block.y, block.z),
                            format!("Chunk: {} {} {}", chunk.x, chunk.y, chunk.z),
                            format!(
                                "Facing: {} ({:.1} / {:.1})",
                                facing,
                                (yaw + 180.).rem_euclid(360.) - 180.,
                                -self.camera.pitch.to_degrees()
                            ),
                        ];
                        if let Some(snapshot) =
                            self.remote.as_ref().and_then(|r| r.snapshot.as_ref())
                        {
                            gpu.ui.debug_left.push(format!(
                                "Client Chunk Cache: {}",
                                snapshot.report.world.chunks.len()
                            ));
                            gpu.ui.debug_left.push(format!(
                                "E: {} loaded",
                                snapshot.report.entities.entities.len()
                            ));
                            let light = snapshot.report.lights.sample([block.x, block.y, block.z]);
                            gpu.ui.debug_left.push(format!(
                                "Server light: sky {:?}, block {:?}; {} sections",
                                light[0], light[1], snapshot.report.lights.section_count
                            ));
                            let environment = &snapshot.report.environment;
                            gpu.ui.debug_left.push(format!(
                                "Server weather: rain {:.3}, thunder {:.3}",
                                environment.rain_level, environment.thunder_level
                            ));
                            if let Some(brand) = &snapshot.report.server_brand {
                                gpu.ui.debug_left.push(format!("Server brand: {brand}"));
                            }
                            if environment.time_received.is_some() {
                                gpu.ui
                                    .debug_left
                                    .push(format!("Game time: {}", environment.game_time));
                                if let Some(clock) = environment
                                    .clock(&snapshot.report.registries, "minecraft:overworld")
                                {
                                    gpu.ui.debug_left.push(format!(
                                        "World clock: {} + {:.3}, rate {:.3}",
                                        clock.total_ticks, clock.partial_tick, clock.rate
                                    ));
                                }
                            }
                            if let Some(distance) = environment.simulation_distance {
                                gpu.ui
                                    .debug_left
                                    .push(format!("Server simulation distance: {distance}"));
                            }
                            if let Some((difficulty, locked)) = environment.difficulty {
                                let name =
                                    ["Peaceful", "Easy", "Normal", "Hard"][difficulty as usize];
                                gpu.ui.debug_left.push(format!(
                                    "Server difficulty: {name}{}",
                                    if locked { " (locked)" } else { "" }
                                ));
                            }
                            if let Some(center) = environment.chunk_center {
                                gpu.ui.debug_left.push(format!(
                                    "Chunk cache center: {} {} / radius {}",
                                    center[0],
                                    center[1],
                                    environment.chunk_radius.unwrap_or(0)
                                ));
                            }
                            if let Some(spawn) = &environment.default_spawn {
                                gpu.ui.debug_left.push(format!(
                                    "World spawn: {} {} {} ({})",
                                    spawn.position[0],
                                    spawn.position[1],
                                    spawn.position[2],
                                    spawn.dimension
                                ));
                            }
                            if let Some(spawn) = &snapshot.report.world.spawn {
                                gpu.ui.debug_left.push(spawn.dimension.clone());
                            }
                        }
                        gpu.ui.debug_right = vec![
                            format!("Display: {}x{}", gpu.config.width, gpu.config.height),
                            gpu.adapter.clone(),
                            format!("Vulkan / {:?}", gpu.config.present_mode),
                            format!(
                                "Scene: {}x{} {}",
                                gpu.scene_size[0],
                                gpu.scene_size[1],
                                self.settings.quality.label()
                            ),
                            format!("{} triangles", gpu.ui.last_triangles),
                            format!("CPU architecture: {}", std::env::consts::ARCH),
                        ];
                    }
                    actions = gpu.ui.prepare(
                        &gpu.device,
                        [gpu.config.width, gpu.config.height],
                        &mut self.settings,
                    );
                }
                for action in actions {
                    match action {
                        ui::Action::Resume => {
                            self.keys.clear();
                            self.capture(true);
                        }
                        ui::Action::MenuClick {
                            id,
                            serial,
                            state,
                            click,
                        } => {
                            if let Some(remote) = &self.remote {
                                if let Err(error) =
                                    remote.request(crate::net::request::Request::MenuClick {
                                        id,
                                        serial,
                                        state,
                                        click,
                                    })
                                {
                                    eprintln!("Menu input: {error:#}");
                                }
                            }
                        }
                        ui::Action::CloseMenu { id, serial } => {
                            if let Some(remote) = &self.remote {
                                if let Err(error) = remote
                                    .request(crate::net::request::Request::CloseMenu { id, serial })
                                {
                                    eprintln!("Closing menu: {error:#}");
                                }
                            }
                        }
                        ui::Action::SignUpdate { epoch, request } => {
                            if let Some(remote) = &self.remote {
                                if let Err(error) = remote.request_at(epoch, request) {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.ui.chat_error(error.to_string());
                                    }
                                }
                            }
                        }
                        ui::Action::Chat(text) => {
                            if let Some(remote) = &self.remote {
                                if let Err(error) =
                                    remote.request(crate::net::request::Request::Chat(text))
                                {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.ui.chat_error(error.to_string());
                                    }
                                }
                            }
                        }
                        ui::Action::Suggest { id, input } => {
                            if let Some(remote) = &self.remote {
                                if let Err(error) =
                                    remote.request(crate::net::request::Request::Suggestions {
                                        id,
                                        input,
                                    })
                                {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.ui.chat_error(error.to_string());
                                    }
                                }
                            }
                        }
                        ui::Action::Title => {
                            if self.remote.is_some() {
                                self.end_remote(ui::Screen::Main);
                            }
                            self.player = None;
                            self.camera.direction_override = None;
                            self.camera.bob = Mat4::IDENTITY;
                            self.camera.fov = 70.;
                            self.keys.clear();
                            self.capture(false);
                        }
                        ui::Action::Quit => event_loop.exit(),
                        ui::Action::Disconnect => self.end_remote(ui::Screen::Multiplayer),
                        ui::Action::Connect(server, username) => {
                            let result = crate::net::Address::parse(&server)
                                .and_then(|address| self.begin_remote(address, username));
                            if let Err(error) = result {
                                self.end_remote(ui::Screen::Disconnected);
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.ui.message = format!("Ошибка: {error}");
                                }
                            }
                        }
                        ui::Action::Changed => {
                            if let Err(error) = self.settings.save(&self.cache) {
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.ui.message = format!("Не удалось сохранить: {error}");
                                }
                            }
                        }
                    }
                }
                if self.remote.is_some() {
                    let was_in_world = self.gpu.as_ref().is_some_and(|gpu| gpu.ui.in_world);
                    let was_container = self.gpu.as_ref().is_some_and(|gpu| {
                        matches!(
                            gpu.ui.screen,
                            ui::Screen::Container | ui::Screen::SignEditor
                        )
                    });
                    let cancelled = self.gpu.as_ref().is_some_and(|gpu| {
                        matches!(gpu.ui.screen, ui::Screen::Main | ui::Screen::Multiplayer)
                    });
                    if cancelled {
                        self.end_remote(ui::Screen::Multiplayer);
                    } else if let Err(error) = self.poll_remote() {
                        self.end_remote(ui::Screen::Disconnected);
                        if let Some(gpu) = &mut self.gpu {
                            gpu.ui.rich_reason = error
                                .downcast_ref::<crate::net::component::Disconnect>()
                                .map(|e| e.0.clone());
                            gpu.ui.message = if let Some(reason) = &gpu.ui.rich_reason {
                                reason.plain_text()
                            } else {
                                format!("Соединение закрыто: {error}")
                            };
                        }
                        if self.screenshot.is_some() || self.frame_limit.is_some() {
                            self.error = Some(error);
                            event_loop.exit();
                        }
                    }
                    if self
                        .gpu
                        .as_ref()
                        .is_some_and(|gpu| gpu.ui.screen == ui::Screen::Connecting)
                    {
                        self.capture(false);
                    } else if (!was_in_world || was_container)
                        && self.gpu.as_ref().is_some_and(|gpu| {
                            gpu.ui.in_world && gpu.ui.screen == ui::Screen::Playing
                        })
                    {
                        self.capture(true);
                    }
                }
                if self.captured
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        matches!(
                            gpu.ui.screen,
                            ui::Screen::Container | ui::Screen::SignEditor
                        )
                    })
                {
                    self.capture(false);
                }
                let mut interaction_ticks = 0u8;
                if let Some(player) = &mut self.player {
                    let active = self.gpu.as_ref().is_some_and(|gpu| {
                        if self.remote.is_some() {
                            gpu.ui.in_world
                        } else {
                            gpu.ui.screen == ui::Screen::Playing
                        }
                    });
                    let down = |key| self.captured && self.keys.contains(&key);
                    let input = Input {
                        forward: down(KeyCode::KeyW) as u8 as f64
                            - down(KeyCode::KeyS) as u8 as f64,
                        strafe: down(KeyCode::KeyD) as u8 as f64 - down(KeyCode::KeyA) as u8 as f64,
                        jump: down(KeyCode::Space),
                        sneak: down(KeyCode::ShiftLeft) || down(KeyCode::ShiftRight),
                        sprint: down(KeyCode::ControlLeft) || down(KeyCode::ControlRight),
                    };
                    if let Some(remote) = &self.remote {
                        if let Some(snapshot) = &remote.snapshot {
                            let collision = crate::world::collision::RemoteCollision {
                                world: &snapshot.report.world,
                                physics: assets::collision::get()
                                    .expect("validated collision data"),
                            };
                            let bits = down(KeyCode::KeyW) as u8
                                | ((down(KeyCode::KeyS) as u8) << 1)
                                | ((down(KeyCode::KeyA) as u8) << 2)
                                | ((down(KeyCode::KeyD) as u8) << 3)
                                | ((input.jump as u8) << 4)
                                | ((input.sneak as u8) << 5)
                                | ((input.sprint as u8) << 6);
                            let mut failure = None;
                            if active && collision.loaded(player.feet) && snapshot.position_ready {
                                player.advance_with(
                                    dt as f64,
                                    input,
                                    self.camera.yaw,
                                    &collision,
                                    |player| {
                                        interaction_ticks = interaction_ticks.saturating_add(1);
                                        let motion = crate::net::movement::Motion {
                                            world_epoch: snapshot.world_epoch,
                                            teleport_version: snapshot.teleport_version,
                                            abilities_version: snapshot.abilities_version,
                                            velocity_version: snapshot.velocity_version,
                                            player_state_version: snapshot.player_state_version,
                                            pose: crate::net::entity::Pose {
                                                position: player.feet,
                                                velocity: player.velocity,
                                                yaw: self.camera.yaw.to_degrees() - 180.,
                                                pitch: -self.camera.pitch.to_degrees(),
                                            },
                                            on_ground: player.grounded,
                                            horizontal_collision: player.horizontal_collision,
                                            input: bits,
                                            sprinting: player.sprinting,
                                            slot: self
                                                .gpu
                                                .as_ref()
                                                .map(|gpu| gpu.ui.inventory.selected as u8)
                                                .unwrap_or(0),
                                            flying: player.abilities.flying,
                                        };
                                        if let Err(error) = remote.send_motion(motion) {
                                            failure = Some(error);
                                        }
                                    },
                                );
                            }
                            if let Some(error) = failure {
                                self.error = Some(error);
                                event_loop.exit();
                            }
                            let (eye, direction) =
                                player.camera(self.camera.yaw, self.camera.pitch, &collision);
                            self.camera.eye = eye;
                            self.camera.direction_override = Some(direction);
                        }
                    } else {
                        if active {
                            player.advance(dt as f64, input, self.camera.yaw, &DemoCollision);
                        }
                        let (eye, direction) =
                            player.camera(self.camera.yaw, self.camera.pitch, &DemoCollision);
                        self.camera.eye = eye;
                        self.camera.direction_override = Some(direction);
                    }
                    self.camera.bob =
                        if player.perspective == Perspective::First && self.settings.view_bobbing {
                            player.bob_matrix_scaled(self.zoom.bob_scale(&self.settings.zoom))
                        } else {
                            Mat4::IDENTITY
                        };
                    self.camera.fov = self.settings.fov as f32
                        * (player.previous_fov
                            + (player.fov_multiplier - player.previous_fov) * player.alpha());
                } else if self.captured && self.remote.is_none() {
                    self.camera.update(&self.keys, dt);
                }
                if self.attack_held {
                    for _ in 0..interaction_ticks {
                        if let Err(error) = self.interact_world_inner(true, true) {
                            eprintln!("Continue mining: {error:#}");
                            break;
                        }
                    }
                }
                self.use_delay = self.use_delay.saturating_sub(interaction_ticks);
                if self.use_held && self.use_delay == 0 {
                    if let Err(error) = self.interact_world(false) {
                        eprintln!("Block use: {error:#}");
                    }
                    self.use_delay = 4;
                }
                self.frame_pacer
                    .rendered(now, self.settings.frame_interval());
                let tile = (
                    (self.camera.eye.x / 32.).floor() as i32,
                    (self.camera.eye.z / 32.).floor() as i32,
                );
                let needs_terrain = self
                    .gpu
                    .as_ref()
                    .is_some_and(|gpu| gpu.ui.in_world || !gpu.panorama_valid);
                if self.remote.is_none()
                    && needs_terrain
                    && tile != self.last_tile
                    && self.request.try_send(self.camera.eye).is_ok()
                {
                    self.last_tile = tile;
                }
                let anchor = self
                    .player
                    .as_ref()
                    .map_or(self.camera.eye, |p| p.interpolated_feet().as_vec3());
                let in_world = self.gpu.as_ref().is_some_and(|g| g.ui.in_world);
                let mut rendered_camera = self.camera_motion.step(
                    &self.camera,
                    anchor,
                    camera_dt,
                    self.settings.camera_smoothing && in_world && self.player.is_some(),
                );
                if let Some(player) = self.player.as_ref().filter(|_| {
                    self.settings.camera_smoothing
                        && in_world
                        && rendered_camera.eye.distance_squared(self.camera.eye) > 1e-6
                }) {
                    let origin = player.eye().as_dvec3();
                    let clip = |world: &dyn crate::player::CollisionWorld| {
                        crate::world::raycast::camera_segment(
                            world,
                            origin,
                            rendered_camera.eye.as_dvec3(),
                        )
                    };
                    let hit = if let Some(snapshot) =
                        self.remote.as_ref().and_then(|r| r.snapshot.as_ref())
                    {
                        let world = crate::world::collision::RemoteCollision {
                            world: &snapshot.report.world,
                            physics: assets::collision::get().expect("validated collision data"),
                        };
                        clip(&world)
                    } else {
                        clip(&DemoCollision)
                    };
                    rendered_camera.eye = hit.as_vec3();
                }
                let zoom_divisor = self.zoom.frame(camera_dt as f64, &self.settings.zoom);
                rendered_camera.fov /= zoom_divisor as f32;
                if let Some(gpu) = &mut self.gpu {
                    if self.remote.is_none() && gpu.pending.is_none() {
                        if let Ok(meshes) = self.response.try_recv() {
                            gpu.pending = Some((meshes.into_iter(), vec![]));
                        }
                    }
                    let capture =
                        if (!gpu.ui.in_world || !gpu.meshes.is_empty() || self.remote.is_some())
                            && gpu.pending.is_none()
                            && gpu.texture_upload.is_none()
                            && (self.remote.is_none() || gpu.ui.in_world)
                            && self
                                .frame_limit
                                .is_none_or(|limit| self.measured_frames + 1 >= limit)
                        {
                            self.screenshot.as_deref()
                        } else {
                            None
                        };
                    match gpu.draw(
                        &rendered_camera,
                        self.radius,
                        self.debug,
                        capture,
                        &self.settings,
                        self.player.as_ref(),
                    ) {
                        Ok((draws, triangles)) => {
                            gpu.ui.last_draws = draws;
                            gpu.ui.last_triangles = triangles;
                            self.frames += 1;
                            if let Some(limit) = self.frame_limit.filter(|_| {
                                gpu.ui.in_world
                                    && (!gpu.meshes.is_empty() || self.remote.is_some())
                                    && gpu.pending.is_none()
                                    && gpu.texture_upload.is_none()
                            }) {
                                let start = self.measurement_start.get_or_insert(Instant::now());
                                self.measured_frames += 1;
                                if self.measured_frames >= limit {
                                    println!("Frame pacing: {:.2} FPS over {} frames; cap {}; mode {:?}; quality {}",(self.measured_frames-1) as f64/start.elapsed().as_secs_f64(),self.measured_frames,self.settings.fps_limit,gpu.config.present_mode,self.settings.quality.label());
                                    event_loop.exit();
                                    self.frame_limit = None;
                                }
                            }
                            if capture.is_some() {
                                self.screenshot = None;
                                event_loop.exit();
                            }
                            if now.duration_since(self.title_time) > Duration::from_secs(1) {
                                let fps = self.frames as f32
                                    / now.duration_since(self.title_time).as_secs_f32();
                                gpu.ui.fps = fps;
                                if let Some(window) = &self.window {
                                    let limit = if self.settings.fps_limit == 0 {
                                        "∞".into()
                                    } else {
                                        self.settings.fps_limit.to_string()
                                    };
                                    let vsync = presentation::label(
                                        self.settings.vsync,
                                        gpu.config.present_mode,
                                    );
                                    let mode = if gpu.ui.in_world {
                                        format!(
                                            "{draws}/{} tiles · {triangles} tris",
                                            gpu.meshes.len()
                                        )
                                    } else {
                                        "Menu".into()
                                    };
                                    window.set_title(&format!("OptiMC · Vulkan · {fps:.0} FPS / {limit} · VSync {vsync} · {} · {mode}",self.settings.quality.label()));
                                }
                                self.frames = 0;
                                self.title_time = now;
                            }
                        }
                        Err(error) => match error.downcast_ref::<wgpu::SurfaceError>() {
                            Some(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                gpu.resize(PhysicalSize::new(gpu.config.width, gpu.config.height))
                            }
                            Some(wgpu::SurfaceError::Timeout) => {}
                            _ => {
                                self.error = Some(error);
                                event_loop.exit();
                            }
                        },
                    }
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            if window.is_minimized().unwrap_or(false) {
                event_loop.set_control_flow(ControlFlow::Wait);
                return;
            }
            if Instant::now() >= self.frame_pacer.deadline {
                window.request_redraw();
            }
            event_loop.set_control_flow(if self.settings.fps_limit == 0 {
                ControlFlow::Poll
            } else {
                ControlFlow::WaitUntil(self.frame_pacer.deadline)
            });
        }
    }
}
#[derive(Default)]
pub struct Launch {
    pub closeup: bool,
    pub frames: Option<u32>,
    pub player: bool,
    pub inventory: bool,
    pub perspective: Perspective,
    pub preview: Option<Preview>,
    /// Development-only visual inspection of received server chunks.
    pub remote_preview: Option<crate::world::remote::RemoteWorld>,
    pub server: Option<(crate::net::Address, String)>,
}
pub fn run(
    cache: PathBuf,
    radius: i32,
    screenshot: Option<PathBuf>,
    start_screen: ui::Screen,
    quality: Option<Quality>,
    launch: Launch,
) -> Result<()> {
    // Validate before opening the window so missing assets produce a readable error.
    assets::ensure_gui(&cache)?;
    assets::texture_pixels(&cache)?;
    assets::collision::get()?;
    assets::collision::outline()?;
    crate::world::entity_pick::prepare();
    crate::net::item_defaults::get()?;
    crate::world::mining::blocks()?;
    let scene = launch
        .preview
        .as_ref()
        .map(|request| model_scene::ModelScene::load(&cache, request))
        .transpose()?;
    let scene = if let Some(world) = &launch.remote_preview {
        Some(model_scene::ModelScene::from_remote(&cache, world)?)
    } else {
        scene
    }
    .map(Arc::new);
    let remote_eye = launch
        .remote_preview
        .as_ref()
        .and(scene.as_ref())
        .map(|scene| {
            let min = scene
                .meshes
                .iter()
                .fold(Vec3::splat(f32::INFINITY), |b, m| b.min(m.min));
            let max = scene
                .meshes
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |b, m| b.max(m.max));
            let distance = (max - min).max_element().max(2.);
            (min + max) * 0.5 + Vec3::new(distance, distance * 0.65, distance)
        });
    let stream_scene = scene.clone();
    let (request, requests) = mpsc::sync_channel::<Vec3>(1);
    let (responses, response) = mpsc::sync_channel(1);
    let radius = radius.clamp(256, 4096);
    let radius = (radius as u32).next_power_of_two() as i32;
    thread::Builder::new()
        .name("terrain-stream".into())
        .spawn(move || {
            let mut streamer = Streamer::new(radius);
            while let Ok(mut eye) = requests.recv() {
                while let Ok(new) = requests.try_recv() {
                    eye = new;
                }
                let start = Instant::now();
                let meshes = if let Some(scene) = &stream_scene {
                    scene.meshes.clone()
                } else {
                    streamer.update(eye)
                };
                println!(
                    "Terrain: {} tiles, {} triangles, {:.0} ms",
                    meshes.len(),
                    meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>(),
                    start.elapsed().as_secs_f64() * 1000.
                );
                if responses.send(meshes).is_err() {
                    break;
                }
            }
        })?;
    let now = Instant::now();
    let mut settings = Settings::load(&cache);
    if let Some(quality) = quality {
        settings.quality = quality;
    }
    let mut app = App {
        cache,
        radius,
        window: None,
        gpu: None,
        scene: scene.clone(),
        remote: None,
        remote_frame: None,
        remote_epoch: 0,
        remote_teleport: 0,
        remote_velocity: u64::MAX,
        remote_abilities: u64::MAX,
        remote_selected_slot: u64::MAX,
        remote_visible: false,
        remote_geometry: None,
        launch_server: launch.server,
        camera: Camera {
            eye: if let Some(eye) = remote_eye {
                eye
            } else if scene.is_some() {
                Vec3::new(2.8, 2., 2.8)
            } else if launch.closeup {
                Vec3::new(30., 63., 58.)
            } else {
                Vec3::new(0., 85., 100.)
            },
            yaw: if scene.is_some() {
                -std::f32::consts::FRAC_PI_4
            } else {
                0.3
            },
            pitch: if scene.is_some() {
                -0.43
            } else if std::env::var_os("OPTIMC_BENCH_SKY").is_some() {
                1.5
            } else {
                -0.3
            },
            speed: if scene.is_some() { 2. } else { 32. },
            direction_override: None,
            bob: Mat4::IDENTITY,
            fov: 70.,
        },
        player: launch.player.then(|| {
            let mut player =
                Player::spawn(DVec3::new(0.5, crate::world::height(0, 100) as f64, 100.5));
            player.perspective = launch.perspective;
            player
        }),
        keys: HashSet::new(),
        captured: false,
        attack_held: false,
        mining: Default::default(),
        use_held: false,
        use_delay: 0,
        hidden_cursor: None,
        debug: false,
        request,
        response,
        last_tile: (i32::MAX, i32::MAX),
        last_frame: now,
        title_time: now,
        frames: 0,
        error: None,
        screenshot,
        settings,
        start_screen: if launch.inventory {
            ui::Screen::Inventory
        } else {
            start_screen
        },
        frame_pacer: pacing::Pacer::new(now),
        hotbar_wheel: crate::scroll::Wheel::default(),
        zoom: Default::default(),
        camera_motion: Default::default(),
        frame_limit: launch.frames,
        measured_frames: 0,
        measurement_start: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod panorama_tests {
    use super::*;
    #[test]
    fn panorama_keeps_rotating_after_long_runtime_without_a_world_camera() {
        let long = Duration::from_secs(365 * 24 * 3600);
        let first = Camera::panorama(long);
        let next = Camera::panorama(long + Duration::from_millis(16));
        assert!(first.direction().distance(next.direction()) > 0.0001);
        assert_eq!(first.eye, Vec3::ZERO);
        assert!(first.direction_override.is_none());
        assert_eq!(first.fov, 85.);
        let before = Camera::panorama(Duration::from_millis(179999)).direction();
        let after = Camera::panorama(Duration::from_millis(180001)).direction();
        assert!(before.distance(after) < 0.001);
    }
}
