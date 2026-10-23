@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;
@group(0) @binding(2) var favicons: texture_2d<f32>;
struct In { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) atlas: u32 };
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32>, @location(2) @interpolate(flat) atlas: u32 };
@vertex fn vs_main(v: In) -> Out {var o:Out;o.position=vec4<f32>(v.position,0.0,1.0);o.uv=v.uv;o.color=v.color;o.atlas=v.atlas;return o;}
@fragment fn fs_main(v: Out) -> @location(0) vec4<f32> {let gui=textureSample(atlas,nearest,v.uv);let icon=textureSample(favicons,nearest,v.uv);return select(gui,icon,v.atlas==1u)*v.color;}
