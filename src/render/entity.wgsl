struct EntityInstance {
    @location(6) root0:vec4<f32>, @location(7) root1:vec4<f32>,
    @location(8) root2:vec4<f32>, @location(9) root3:vec4<f32>,
    @location(10) head0:vec4<f32>, @location(11) head1:vec4<f32>,
    @location(12) head2:vec4<f32>, @location(13) head3:vec4<f32>,
};
@vertex fn vs_entity(v:Input, instance:EntityInstance)->Output {
    let root=mat4x4<f32>(instance.root0,instance.root1,instance.root2,instance.root3);
    let head=mat4x4<f32>(instance.head0,instance.head1,instance.head2,instance.head3);
    var transform=root;
    if v.lod==1u {transform=root*head;}
    let world=transform*vec4<f32>(v.position,1.0);
    let normal=normalize((transform*vec4<f32>(v.normal,0.0)).xyz);
    var o:Output;o.clip=camera.view_proj*world;o.uv=v.uv;o.layer=v.layer;
    o.world=world.xyz;o.normal=normal;o.lod=1u;
    let light0=normalize(vec3<f32>(0.2,1.0,-0.7));let light1=normalize(vec3<f32>(-0.2,1.0,0.7));
    let light=min(1.0,(max(dot(light0,normal),0.0)+max(dot(light1,normal),0.0))*0.6+0.4);
    o.tint=v.tint;o.shade=v.shade*light;return o;
}
