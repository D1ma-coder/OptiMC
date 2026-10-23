use crate::{settings::Quality, world::Vertex};
pub const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub fn shader(device: &wgpu::Device, name: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(
            format!("{}\n{source}", include_str!("common.wgsl")).into(),
        ),
    })
}
pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32x3,3=>Uint32,4=>Uint32,5=>Float32x3,14=>Float32x3,15=>Float32];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}
pub fn shadow(device: &wgpu::Device, quality: Quality) -> wgpu::TextureView {
    let size = quality.shadow_size();
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Directional shadow map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
pub struct Effects {
    pub shadow_bind: wgpu::BindGroup,
    pub hdr: wgpu::TextureView,
    pub sky: wgpu::RenderPipeline,
    pub low_sky: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    post: wgpu::RenderPipeline,
    post_bind: wgpu::BindGroup,
    post_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}
impl Effects {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        common: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Scene layout"),
            bind_group_layouts: &[common],
            push_constant_ranges: &[],
        });
        let sky_shader = shader(device, "Atmosphere + panorama", include_str!("sky.wgsl"));
        let sky_pipeline = |target_format, fragment_entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Sky"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &sky_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &sky_shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let sky = sky_pipeline(HDR, "fs_main");
        let low_sky = sky_pipeline(format, "fs_low");
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow camera only"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&shadow_layout],
            push_constant_ranges: &[],
        });
        let terrain = shader(device, "Shadow caster", include_str!("terrain.wgsl"));
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow depth pass"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &terrain,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[vertex_layout()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.,
                    clamp: 0.,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR post bindings"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR tone map / bloom"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&post_layout],
            push_constant_ranges: &[],
        });
        let post = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Post processing"),
            layout: Some(&post_pl),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let hdr = hdr(device, size);
        let post_bind = post_bind(device, &post_layout, &hdr, &sampler, uniform);
        Self {
            hdr,
            sky,
            low_sky,
            shadow_pipeline,
            shadow_bind,
            post,
            post_bind,
            post_layout,
            sampler,
        }
    }
    pub fn resize(&mut self, device: &wgpu::Device, size: [u32; 2], uniform: &wgpu::Buffer) {
        self.hdr = hdr(device, size);
        self.post_bind = post_bind(device, &self.post_layout, &self.hdr, &self.sampler, uniform);
    }
    pub fn post(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tone map + bloom"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.post);
        pass.set_bind_group(0, &self.post_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
fn hdr(device: &wgpu::Device, size: [u32; 2]) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR scene"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
fn post_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("HDR scene input"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hdr),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn explicit_diffuse_attribute_fits_vertex_bytes_and_instance_attribute_budget() {
        let layout = vertex_layout();
        let shade = layout
            .attributes
            .iter()
            .find(|a| a.shader_location == 15)
            .unwrap();
        assert_eq!(shade.format, wgpu::VertexFormat::Float32);
        assert_eq!(shade.offset, std::mem::offset_of!(Vertex, shade) as u64);
        assert_eq!(shade.offset + 4, layout.array_stride);
        // Entity root/head instances consume locations 6 through 13.
        assert_eq!(layout.attributes.len() + 8, 16);
        assert!(layout
            .attributes
            .iter()
            .all(|a| !(6..14).contains(&a.shader_location)));
    }
}
