struct Camera {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    settings: vec4<f32>,
    fx: vec4<f32>,
    sun: vec4<f32>,
    relative_view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var textures: texture_2d_array<f32>;
@group(0) @binding(2) var texture_sampler: sampler;
@group(0) @binding(3) var shadow_map: texture_depth_2d;
@group(0) @binding(4) var shadow_sampler: sampler_comparison;
@group(0) @binding(5) var panorama: texture_cube<f32>;
@group(0) @binding(6) var panorama_sampler: sampler;
// Shared distance tiers; zero disables shader LOD entirely.
fn detail_level(distance:f32)->f32 {
    let base=camera.fx.w;
    if base<=0.0 {return 0.0;}
    return smoothstep(base*0.9,base*1.1,distance)
        +smoothstep(base*1.8,base*2.2,distance)
        +smoothstep(base*3.6,base*4.4,distance)
        +smoothstep(base*7.2,base*8.8,distance);
}
fn shadow_at(world:vec3<f32>,normal:vec3<f32>,soft:bool) -> f32 {
    let clip=camera.light_view_proj*vec4<f32>(world+normal*0.035,1.0);
    let ndc=clip.xyz/clip.w;
    let uv=vec2<f32>(ndc.x*0.5+0.5,0.5-ndc.y*0.5);
    if any(uv<vec2<f32>(0.002)) || any(uv>vec2<f32>(0.998)) || ndc.z<0.0 || ndc.z>1.0 {return 1.0;}
    let texel=1.0/vec2<f32>(textureDimensions(shadow_map));
    let bias=max(0.00015,0.00045*(1.0-dot(normal,camera.sun.xyz)));
    let detail=detail_level(length(world-camera.eye.xyz));
    let fade=1.0-smoothstep(3.0,4.0,detail);
    if fade<=0.0 {return 1.0;}
    if !soft {return mix(1.0,textureSampleCompareLevel(shadow_map,shadow_sampler,uv,ndc.z-bias),fade);}
    var sum=0.0;var count=0.0;
    let radius=select(1,2,camera.fx.y>1.5);
    for(var x=-2;x<=2;x++){for(var y=-2;y<=2;y++){
        let ring=max(abs(x),abs(y));
        var weight=1.0;
        if ring==2 {weight=1.0-smoothstep(0.0,1.0,detail);}
        if ring==1 {weight=1.0-smoothstep(1.0,2.0,detail);}
        if ring<=radius && weight>0.0 {
            sum+=textureSampleCompareLevel(shadow_map,shadow_sampler,uv+vec2<f32>(f32(x),f32(y))*texel,ndc.z-bias)*weight;count+=weight;
        }
    }}
    return mix(1.0,sum/count,fade*smoothstep(0.0,0.08,min(min(uv.x,uv.y),min(1.0-uv.x,1.0-uv.y))));
}
fn hash(p:vec2<f32>)->f32 {return fract(sin(dot(p,vec2<f32>(127.1,311.7)))*43758.5453);}
fn noise(p:vec2<f32>)->f32 {
    let i=floor(p);let f=fract(p);let u=f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2<f32>(1.0,0.0)),u.x),mix(hash(i+vec2<f32>(0.0,1.0)),hash(i+vec2<f32>(1.0,1.0)),u.x),u.y);
}
fn atmosphere(ray:vec3<f32>)->vec3<f32> {
    let horizon=pow(1.0-max(ray.y,0.0),3.5);
    var color=mix(vec3<f32>(0.10,0.30,0.65),vec3<f32>(0.68,0.78,0.86),horizon);
    let toward=max(dot(ray,camera.sun.xyz),0.0);
    color+=vec3<f32>(1.0,0.65,0.30)*pow(toward,8.0)*0.28;
    color+=vec3<f32>(5.0,4.5,3.4)*smoothstep(0.9991,0.9996,toward);
    if ray.y>0.015 {
        var p=ray.xz/(ray.y+0.13)*2.0+vec2<f32>(camera.fx.x*0.007,0.0);
        var value=0.0;var amplitude=0.5;
        let sky_detail=detail_level(600.0/max(ray.y+0.2,0.2));
        let octaves=max(1.0,f32(select(3,5,camera.fx.y>1.5))-sky_detail);
        for(var i=0;i<5;i++){
            let weight=clamp(octaves-f32(i),0.0,1.0);
            if weight>0.0 {value+=noise(p)*amplitude*weight;}
            p=p*2.03+13.1;amplitude*=0.5;
        }
        let cloud=smoothstep(0.46,0.72,value)*smoothstep(0.015,0.13,ray.y);
        let light=mix(vec3<f32>(0.65,0.72,0.81),vec3<f32>(1.2,1.14,1.0),value);
        color=mix(color,light,cloud*0.86);
    }
    return color;
}
