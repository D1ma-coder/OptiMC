struct Camera {
    view_proj:mat4x4<f32>,inverse_view_proj:mat4x4<f32>,light_view_proj:mat4x4<f32>,
    eye:vec4<f32>,settings:vec4<f32>,fx:vec4<f32>,sun:vec4<f32>,
    relative_view_proj:mat4x4<f32>,
};
@group(0) @binding(0) var scene:texture_2d<f32>;
@group(0) @binding(1) var linear_sampler:sampler;
@group(0) @binding(2) var<uniform> camera:Camera;
struct Out {@builtin(position) clip:vec4<f32>,@location(0) uv:vec2<f32>};
@vertex fn vs_main(@builtin(vertex_index) i:u32)->Out {
    let p=vec2<f32>(f32((i<<1u)&2u),f32(i&2u))*2.0-1.0;
    var o:Out;o.clip=vec4<f32>(p,0.0,1.0);o.uv=vec2<f32>(p.x*0.5+0.5,0.5-p.y*0.5);return o;
}
fn aces(c:vec3<f32>)->vec3<f32> {return clamp((c*(2.51*c+0.03))/(c*(2.43*c+0.59)+0.14),vec3<f32>(0.0),vec3<f32>(1.0));}
@fragment fn fs_main(v:Out)->@location(0) vec4<f32> {
    var color=textureSample(scene,linear_sampler,v.uv).rgb;
    if camera.fx.z>0.5 {
        // A soft panorama backdrop keeps the original pixel GUI easy to read.
        let texel=1.5/vec2<f32>(textureDimensions(scene));
        color=(color*4.0+textureSample(scene,linear_sampler,v.uv+texel).rgb+textureSample(scene,linear_sampler,v.uv-texel).rgb+textureSample(scene,linear_sampler,v.uv+texel*vec2<f32>(1.0,-1.0)).rgb+textureSample(scene,linear_sampler,v.uv+texel*vec2<f32>(-1.0,1.0)).rgb)/8.0;
        return vec4<f32>(color,1.0);
    }
    if camera.fx.y>0.5 {
        let texel=1.0/vec2<f32>(textureDimensions(scene));
        var bloom=vec3<f32>(0.0);var count=0.0;
        let radius=select(1,2,camera.fx.y>1.5);
        for(var x=-2;x<=2;x++){for(var y=-2;y<=2;y++){
            if abs(x)<=radius && abs(y)<=radius {
                let sample=textureSample(scene,linear_sampler,v.uv+vec2<f32>(f32(x),f32(y))*texel*4.0).rgb;
                bloom+=max(sample-vec3<f32>(1.0),vec3<f32>(0.0));count+=1.0;
            }
        }}
        color=aces(color*1.12+bloom/count*select(0.08,0.22,camera.fx.y>1.5));
        if camera.fx.y>1.5 {let edge=dot(v.uv-0.5,v.uv-0.5);color*=1.0-edge*0.08;}
    }
    return vec4<f32>(color,1.0);
}
