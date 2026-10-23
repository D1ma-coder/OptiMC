struct Input {
    @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>,
    @location(2) tint: vec3<f32>, @location(3) layer: u32,
    @location(4) lod: u32, @location(5) normal: vec3<f32>,
    @location(14) origin: vec3<f32>, @location(15) shade: f32,
};
struct Output {
    @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>,
    @location(1) tint: vec3<f32>, @location(2) @interpolate(flat) layer: u32,
    @location(3) world: vec3<f32>, @location(4) @interpolate(flat) lod: u32,
    @location(5) normal: vec3<f32>, @location(6) shade: f32,
};
@vertex fn vs_main(v:Input)->Output {
    var o:Output;let world=v.position+v.origin;o.clip=camera.view_proj*vec4<f32>(world,1.0);
    if (v.lod & 1024u)!=0u {let relative=(v.origin-camera.eye.xyz)+v.position;o.clip=camera.relative_view_proj*vec4<f32>(relative,1.0);}
    o.uv=v.uv;o.tint=v.tint;o.layer=v.layer;o.world=world;o.lod=v.lod;o.normal=v.normal;o.shade=v.shade;return o;
}
@vertex fn vs_shadow(v:Input)->@builtin(position) vec4<f32> {return camera.light_view_proj*vec4<f32>(v.position+v.origin,1.0);}
@fragment fn fs_main(v:Output)->@location(0) vec4<f32> {
    var sample=textureSample(textures,texture_sampler,v.uv,i32(v.layer & 0x7fffffffu));
    if (v.lod & 768u)!=0u {sample=textureSampleLevel(textures,texture_sampler,v.uv,i32(v.layer & 0x7fffffffu),0.0);}
    if sample.a<select(0.1,0.00001,(v.layer & 0x80000000u)!=0u) {discard;}
    let normal=normalize(v.normal);let distance=length(v.world-camera.eye.xyz);
    let material=sample.rgb*v.tint;
    var color=material*v.shade;
    if camera.fx.y>0.5 && (v.lod & 256u)==0u {
        let diffuse=max(dot(normal,camera.sun.xyz),0.0);
        let shadow=shadow_at(v.world,normal,true);
        let ambient=vec3<f32>(0.34,0.43,0.56)*(0.65+0.35*max(normal.y,0.0));
        color=material*(ambient+vec3<f32>(1.45,1.25,0.96)*diffuse*shadow);
        let haze=1.0-exp(-distance*0.00065);
        color=mix(color,atmosphere(normalize(v.world-camera.eye.xyz))*0.68,haze);
        let detail=detail_level(distance);
        let samples=max(0.0,6.0-detail*2.0);
        if camera.fx.y>1.5 && samples>0.0 {
            // Fractional final sample blends tiers instead of flashing on/off.
            let ray=v.world-camera.eye.xyz;var lit=0.0;var weight=0.0;
            for(var i=0;i<6;i++){
                let w=clamp(samples-f32(i),0.0,1.0);
                if w>0.0 {
                    let t=min((f32(i)+0.5)/max(samples,1.0),1.0);
                    lit+=shadow_at(camera.eye.xyz+ray*t,vec3<f32>(0.0),false)*w;weight+=w;
                }
            }
            let optical=1.0-exp(-min(distance,400.0)*0.00065);
            let forward=pow(max(dot(normalize(ray),camera.sun.xyz),0.0),5.0);
            color+=vec3<f32>(1.0,0.78,0.44)*(lit/max(weight,0.0001))*min(samples,1.0)*optical*(0.16+forward*0.65);
        }
    }
    if camera.settings.z>0.5 && (v.lod & 256u)==0u {color=mix(color,0.5+0.5*cos(vec3<f32>(0.0,2.1,4.2)+f32(v.lod)*1.4),0.7);}
    let fog=smoothstep(camera.settings.x,camera.settings.y,distance);
    var fog_color=vec3<f32>(0.48,0.69,0.88);
    if camera.fx.y>0.5 {fog_color=atmosphere(normalize(v.world-camera.eye.xyz))*0.8;}
    return vec4<f32>(mix(color,fog_color,fog),sample.a);
}

// A separate entry point keeps every shadow/atmosphere/bloom dependency out
// of the Low pipeline, rather than relying on runtime uniform branches.
@fragment fn fs_low(v:Output)->@location(0) vec4<f32> {
    var sample=textureSample(textures,texture_sampler,v.uv,i32(v.layer & 0x7fffffffu));
    if (v.lod & 768u)!=0u {sample=textureSampleLevel(textures,texture_sampler,v.uv,i32(v.layer & 0x7fffffffu),0.0);}
    if sample.a<select(0.1,0.00001,(v.layer & 0x80000000u)!=0u) {discard;}
    let distance=length(v.world-camera.eye.xyz);
    var color=sample.rgb*v.tint*v.shade;
    if camera.settings.z>0.5 && (v.lod & 256u)==0u {color=mix(color,0.5+0.5*cos(vec3<f32>(0.0,2.1,4.2)+f32(v.lod)*1.4),0.7);}
    return vec4<f32>(mix(color,vec3<f32>(0.48,0.69,0.88),smoothstep(camera.settings.x,camera.settings.y,distance)),sample.a);
}
