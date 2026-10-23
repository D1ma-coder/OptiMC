//! Source first-person item arm matrices; independent of world camera smoothing.
use glam::{Mat4, Vec3};
fn sine(angle: f32) -> f32 {
    // Mth.sin(double): a quantized 65536-entry table, indexed before sampling.
    let index = ((f64::from(angle) * 10430.378350470453) as i64 & 65535) as f64;
    (index * std::f64::consts::TAU / 65536.).sin() as f32
}
pub(super) fn arm(left: bool, equip: f32, swing: f32) -> Mat4 {
    let sign = if left { -1. } else { 1. };
    let s = swing.clamp(0., 1.);
    let root = s.sqrt();
    let pi = std::f32::consts::PI;
    Mat4::from_translation(Vec3::new(sign * 0.56, -0.52 - equip * 0.6, -0.72))
        * Mat4::from_translation(Vec3::new(
            sign * (-0.4 * sine(root * pi)),
            0.2 * sine(root * std::f32::consts::TAU),
            -0.2 * sine(s * pi),
        ))
        * Mat4::from_rotation_y((sign * (45. - sine(s * s * pi) * 20.)).to_radians())
        * Mat4::from_rotation_z((sign * sine(root * pi) * -20.).to_radians())
        * Mat4::from_rotation_x((sine(root * pi) * -80.).to_radians())
        * Mat4::from_rotation_y((sign * -45.).to_radians())
}
use crate::{
    assets::held_items::{Key, Model, Worker},
    world::Vertex,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;
fn swing_progress(elapsed: Option<Duration>) -> f32 {
    elapsed.map_or(0., |t| {
        let s = t.as_secs_f32() / 0.3;
        if s < 1. {
            s
        } else {
            0.
        }
    })
}
// ItemInHandRenderer.renderPlayerArm's animated prefix, relative to its rest pose.
// The existing skin mesh has its own rest/model origin, so apply the source delta.
fn empty_arm_motion(s: f32) -> Mat4 {
    let s = s.clamp(0., 1.);
    let pi = std::f32::consts::PI;
    let root = |s: f32| {
        Mat4::from_translation(Vec3::new(
            0.64000005 - 0.3 * sine(s.sqrt() * pi),
            -0.6 + 0.4 * sine(s.sqrt() * std::f32::consts::TAU),
            -0.71999997 - 0.4 * sine(s * pi),
        )) * Mat4::from_rotation_y(45f32.to_radians())
            * Mat4::from_rotation_y((70. * sine(s.sqrt() * pi)).to_radians())
            * Mat4::from_rotation_z((-20. * sine(s * s * pi)).to_radians())
    };
    root(s) * root(0.).inverse()
}
pub(super) struct Renderer {
    worker: Worker,
    equipped: [Option<Key>; 2],
    height: [f32; 2],
    previous: [f32; 2],
    tick: Instant,
    swing: Option<Instant>,
    swing_left: bool,
    uniform: wgpu::Buffer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    count: u32,
    bind: Option<wgpu::BindGroup>,
    material_keys: Vec<Key>,
    quality: Option<crate::settings::Quality>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    pub visible: [bool; 2],
}
pub(super) struct Resources<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub layout: &'a wgpu::BindGroupLayout,
    pub sampler: &'a wgpu::Sampler,
    pub shadow: &'a wgpu::TextureView,
    pub shadow_sampler: &'a wgpu::Sampler,
    pub panorama: &'a wgpu::TextureView,
    pub panorama_sampler: &'a wgpu::Sampler,
}
impl Renderer {
    pub fn new(device: &wgpu::Device, cache: &Path) -> anyhow::Result<Self> {
        let worker = Worker::new(cache)?;
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("First-person camera"),
            size: std::mem::size_of::<super::Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Held item original textures"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let vertex = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Held item vertices"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Held item indices"),
            size: 4,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            worker,
            equipped: [None, None],
            height: [1.; 2],
            previous: [1.; 2],
            tick: Instant::now(),
            swing: None,
            swing_left: false,
            uniform,
            texture,
            view,
            vertex,
            index,
            vertex_capacity: 4,
            index_capacity: 4,
            count: 0,
            bind: None,
            material_keys: Vec::new(),
            quality: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            visible: [false; 2],
        })
    }
    pub fn swing(&mut self) {
        self.swing_hand(false);
    }
    pub fn swing_hand(&mut self, left: bool) {
        let now = Instant::now();
        if self
            .swing
            .is_none_or(|at| now.duration_since(at).as_secs_f32() >= 0.15)
        {
            self.swing = Some(now);
            self.swing_left = left;
        }
    }
    pub fn swing_progress(&self) -> f32 {
        swing_progress(self.swing.map(|at| at.elapsed()))
    }
    pub fn empty_arm_motion(&self) -> Mat4 {
        if self.swing_left {
            Mat4::IDENTITY
        } else {
            empty_arm_motion(self.swing_progress())
        }
    }
    pub fn reset(&mut self) {
        self.equipped = [None, None];
        self.height = [1.; 2];
        self.previous = [1.; 2];
        self.tick = Instant::now();
        self.swing = None;
        self.visible = [false; 2];
        self.count = 0;
    }
    pub fn prepare(
        &mut self,
        resources: Resources<'_>,
        keys: [Option<Key>; 2],
        size: [u32; 2],
        quality: crate::settings::Quality,
        bob: Mat4,
    ) {
        let now = Instant::now();
        let step = Duration::from_millis(50);
        let mut ticks = 0;
        while now.saturating_duration_since(self.tick) >= step && ticks < 10 {
            self.tick += step;
            ticks += 1;
            self.previous = self.height;
            for (i, key) in keys.iter().enumerate() {
                let target = if key == &self.equipped[i] { 1. } else { 0. };
                self.height[i] += (target - self.height[i]).clamp(-0.4, 0.4);
                if self.height[i] < 0.1 {
                    self.equipped[i] = key.clone();
                }
            }
        }
        if ticks == 10 {
            self.tick = now;
        }
        for key in keys.iter().flatten() {
            self.worker.get(key);
        }
        let models: [Option<Arc<Model>>; 2] = std::array::from_fn(|i| {
            self.equipped[i]
                .as_ref()
                .and_then(|key| self.worker.get(key))
        });
        self.visible = std::array::from_fn(|i| models[i].is_some());
        self.vertices.clear();
        self.indices.clear();
        let material_keys: Vec<Key> = (0..2)
            .filter(|&i| models[i].is_some())
            .filter_map(|i| self.equipped[i].clone())
            .collect();
        let reload = material_keys != self.material_keys;
        let mut offset = 0;
        let alpha = (now.saturating_duration_since(self.tick).as_secs_f32() * 20.).clamp(0., 1.);
        let swing = self.swing_progress();
        for (i, model) in models
            .into_iter()
            .enumerate()
            .filter_map(|(i, m)| m.map(|m| (i, m)))
        {
            let height = self.previous[i] + (self.height[i] - self.previous[i]) * alpha;
            let transform = arm(
                i == 1,
                1. - height,
                if (i == 1) == self.swing_left {
                    swing
                } else {
                    0.
                },
            );
            if reload {
                for (layer, pixels) in model.textures.iter().enumerate() {
                    let image = image::imageops::resize(
                        pixels,
                        64,
                        64,
                        image::imageops::FilterType::Nearest,
                    );
                    resources.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: offset + layer as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        image.as_raw(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(256),
                            rows_per_image: Some(64),
                        },
                        wgpu::Extent3d {
                            width: 64,
                            height: 64,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
            let base = self.vertices.len() as u32;
            self.indices
                .extend(model.mesh.indices.iter().map(|v| v + base));
            self.vertices.extend(model.mesh.vertices.iter().map(|v| {
                let mut v = *v;
                v.position = transform
                    .transform_point3(Vec3::from_array(v.position))
                    .to_array();
                let normal = transform
                    .transform_vector3(Vec3::from_array(v.normal))
                    .normalize_or_zero();
                v.normal = normal.to_array();
                v.layer += offset;
                let light0 = Vec3::new(0.2, 1., -0.7).normalize();
                let light1 = Vec3::new(-0.2, 1., 0.7).normalize();
                let light =
                    (normal.dot(light0).max(0.) + normal.dot(light1).max(0.)).min(1.) * 0.6 + 0.4;
                v.shade = light;
                v
            }));
            offset += model.textures.len() as u32;
        }
        self.material_keys = material_keys;
        self.count = self.indices.len() as u32;
        if self.count == 0 {
            return;
        }
        let vertex_bytes = bytemuck::cast_slice::<Vertex, u8>(&self.vertices);
        let index_bytes = bytemuck::cast_slice::<u32, u8>(&self.indices);
        if vertex_bytes.len() > self.vertex_capacity {
            self.vertex = resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Held item vertices"),
                    contents: vertex_bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
            self.vertex_capacity = vertex_bytes.len();
        } else {
            resources.queue.write_buffer(&self.vertex, 0, vertex_bytes);
        }
        if index_bytes.len() > self.index_capacity {
            self.index = resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Held item indices"),
                    contents: index_bytes,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });
            self.index_capacity = index_bytes.len();
        } else {
            resources.queue.write_buffer(&self.index, 0, index_bytes);
        }
        let projection = Mat4::perspective_rh(
            70f32.to_radians(),
            size[0] as f32 / size[1].max(1) as f32,
            0.01,
            100.,
        ) * bob;
        let matrix = projection.to_cols_array_2d();
        let uniform = super::Uniform {
            matrix,
            inverse_matrix: projection.inverse().to_cols_array_2d(),
            light_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            eye: [0.; 4],
            settings: [1000., 1001., 0., 0.],
            fx: [0.; 4],
            sun: [0., 1., 0., 0.],
            relative_matrix: matrix,
        };
        resources
            .queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        if self.bind.is_none() || self.quality != Some(quality) {
            self.bind = Some(
                resources
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Held item material camera"),
                        layout: resources.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: self.uniform.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&self.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(resources.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::TextureView(resources.shadow),
                            },
                            wgpu::BindGroupEntry {
                                binding: 4,
                                resource: wgpu::BindingResource::Sampler(resources.shadow_sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: wgpu::BindingResource::TextureView(resources.panorama),
                            },
                            wgpu::BindGroupEntry {
                                binding: 6,
                                resource: wgpu::BindingResource::Sampler(
                                    resources.panorama_sampler,
                                ),
                            },
                        ],
                    }),
            );
            self.quality = Some(quality);
        }
    }
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
    ) {
        if self.count == 0 {
            return;
        }
        let Some(bind) = &self.bind else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("First-person held items"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.vertex.slice(..));
        pass.set_index_buffer(self.index.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.count, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    #[ignore = "Requires imported official item resources; CPU only"]
    fn source_item_geometry_reaches_first_person_projection_without_a_gpu() {
        use image::Pixel;
        let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut baker = crate::assets::held_items::Baker::new(&cache);
        for name in [
            "minecraft:compass",
            "minecraft:iron_pickaxe",
            "minecraft:stone",
        ] {
            let item = crate::net::item_defaults::get()
                .unwrap()
                .iter()
                .find(|v| v.name == name)
                .unwrap();
            let model = baker
                .bake(&Key {
                    item: item.id,
                    model: None,
                    left: false,
                })
                .unwrap();
            let projection =
                Mat4::perspective_rh(70f32.to_radians(), 16. / 9., 0.01, 100.) * arm(false, 0., 0.);
            let mut image = image::RgbaImage::new(640, 360);
            let mut depth = vec![1f32; 640 * 360];
            for tri in model.mesh.indices.as_chunks::<3>().0 {
                let vertices = tri
                    .iter()
                    .map(|&i| model.mesh.vertices[i as usize])
                    .collect::<Vec<_>>();
                let clip: [glam::Vec4; 3] = std::array::from_fn(|i| {
                    projection * Vec3::from_array(vertices[i].position).extend(1.)
                });
                if clip.iter().any(|v| v.w <= 0.) {
                    continue;
                }
                let points =
                    clip.map(|v| glam::Vec2::new((v.x / v.w + 1.) * 320., (1. - v.y / v.w) * 180.));
                let edge = |a: glam::Vec2, b: glam::Vec2, p: glam::Vec2| (b - a).perp_dot(p - a);
                let area = edge(points[0], points[1], points[2]);
                if area.abs() < 1e-6 {
                    continue;
                }
                let min = points
                    .into_iter()
                    .fold(glam::Vec2::splat(f32::INFINITY), |a, b| a.min(b))
                    .floor()
                    .max(glam::Vec2::ZERO);
                let max = points
                    .into_iter()
                    .fold(glam::Vec2::splat(f32::NEG_INFINITY), |a, b| a.max(b))
                    .ceil()
                    .min(glam::Vec2::new(639., 359.));
                if min.x > max.x || min.y > max.y {
                    continue;
                }
                let texture = &model.textures[vertices[0].layer as usize];
                for y in min.y as u32..=max.y as u32 {
                    for x in min.x as u32..=max.x as u32 {
                        let p = glam::Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                        let weights = [
                            edge(points[1], points[2], p) / area,
                            edge(points[2], points[0], p) / area,
                            edge(points[0], points[1], p) / area,
                        ];
                        if weights.iter().any(|&w| w < 0.) {
                            continue;
                        }
                        let z = (0..3)
                            .map(|i| weights[i] * clip[i].z / clip[i].w)
                            .sum::<f32>();
                        let index = (y * 640 + x) as usize;
                        if z < 0. || z >= depth[index] {
                            continue;
                        }
                        let reciprocal = (0..3).map(|i| weights[i] / clip[i].w).sum::<f32>();
                        let uv = (0..3)
                            .map(|i| {
                                glam::Vec2::from_array(vertices[i].uv) * weights[i] / clip[i].w
                            })
                            .sum::<glam::Vec2>()
                            / reciprocal;
                        let tx = (uv.x.rem_euclid(1.) * texture.width() as f32) as u32;
                        let ty = (uv.y.rem_euclid(1.) * texture.height() as f32) as u32;
                        let pixel = texture
                            .get_pixel(tx.min(texture.width() - 1), ty.min(texture.height() - 1));
                        if pixel[3] < 26 {
                            continue;
                        }
                        image.get_pixel_mut(x, y).blend(pixel);
                        depth[index] = z;
                    }
                }
            }
            assert!(
                image.pixels().filter(|p| p[3] > 0).count() > 50,
                "{name} is outside the first-person view"
            );
            image
                .save(cache.join(format!(
                    "held-{}.png",
                    name.strip_prefix("minecraft:").unwrap()
                )))
                .unwrap();
        }
    }
    #[test]
    fn source_hand_swing_and_equip_matrices_match_both_arms() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/held-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for row in reference["hands"].as_array().unwrap() {
            let matrix = arm(
                row["left"].as_bool().unwrap(),
                row["equip"].as_f64().unwrap() as f32,
                row["swing"].as_f64().unwrap() as f32,
            );
            for (i, v) in matrix.to_cols_array().iter().enumerate() {
                assert!(
                    (v - row["matrix"][i].as_f64().unwrap() as f32).abs() < 3e-6,
                    "{row}: {i}: {v}"
                );
            }
        }
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    #[test]
    fn empty_arm_delta_matches_original_renderer_pose_stack() {
        use std::io::Read;
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/held-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let matrix = |row: &serde_json::Value| {
            Mat4::from_cols_array(&std::array::from_fn(|i| {
                row["matrix"][i].as_f64().unwrap() as f32
            }))
        };
        let rows = r["empty_hands"].as_array().unwrap();
        let rest = matrix(&rows[0]);
        for row in rows {
            let expected = matrix(row) * rest.inverse();
            assert!(
                empty_arm_motion(row["swing"].as_f64().unwrap() as f32).abs_diff_eq(expected, 2e-5),
                "{row}"
            );
        }
    }
    #[test]
    fn air_swing_moves_empty_arm_and_returns_to_rest_after_six_ticks() {
        assert_eq!(swing_progress(None), 0.);
        assert!((swing_progress(Some(Duration::from_millis(150))) - 0.5).abs() < 1e-6);
        assert_eq!(swing_progress(Some(Duration::from_millis(300))), 0.);
        assert_eq!(swing_progress(Some(Duration::from_secs(9))), 0.);
        assert!(empty_arm_motion(0.).abs_diff_eq(Mat4::IDENTITY, 1e-6));
        assert!(!empty_arm_motion(0.5).abs_diff_eq(Mat4::IDENTITY, 0.01));
        assert!(
            empty_arm_motion(swing_progress(Some(Duration::from_millis(300))))
                .abs_diff_eq(Mat4::IDENTITY, 1e-6)
        );
    }
}
