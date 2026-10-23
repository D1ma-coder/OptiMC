struct SkyOut {@builtin(position) clip:vec4<f32>,@location(0) ndc:vec2<f32>};
@vertex fn vs_main(@builtin(vertex_index) i:u32)->SkyOut {
    let p=vec2<f32>(f32((i<<1u)&2u),f32(i&2u))*2.0-1.0;
    var o:SkyOut;o.clip=vec4<f32>(p,1.0,1.0);o.ndc=p;return o;
}
@fragment fn fs_main(v:SkyOut)->@location(0) vec4<f32> {
    let point=camera.inverse_view_proj*vec4<f32>(v.ndc,1.0,1.0);
    let ray=normalize(point.xyz/point.w-camera.eye.xyz);
    if camera.fx.z>0.5 && camera.sun.w>0.5 {return vec4<f32>(textureSample(panorama,panorama_sampler,ray).rgb,1.0);}
    if camera.fx.y<0.5 {
        var color=mix(vec3<f32>(0.48,0.69,0.88),vec3<f32>(0.27,0.47,0.80),max(ray.y,0.0)*0.55);
        if dot(ray,camera.sun.xyz)>0.9985 {color=vec3<f32>(1.0,0.98,0.85);}
        return vec4<f32>(color,1.0);
    }
    return vec4<f32>(atmosphere(ray),1.0);
}

@fragment fn fs_low(v:SkyOut)->@location(0) vec4<f32> {
    let point=camera.inverse_view_proj*vec4<f32>(v.ndc,1.0,1.0);
    let ray=normalize(point.xyz/point.w-camera.eye.xyz);
    var color=mix(vec3<f32>(0.48,0.69,0.88),vec3<f32>(0.27,0.47,0.80),max(ray.y,0.0)*0.55);
    if dot(ray,camera.sun.xyz)>0.9985 {color=vec3<f32>(1.0,0.98,0.85);}
    return vec4<f32>(color,1.0);
}
