fn validate(source: &str) {
    let module = naga::front::wgsl::parse_str(source).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}
#[test]
fn terrain_and_shadow_shader_pass_naga_validation() {
    validate(&format!(
        "{}\n{}",
        include_str!("../src/render/common.wgsl"),
        include_str!("../src/render/terrain.wgsl")
    ));
}
#[test]
fn atmospheric_sky_shader_passes_naga_validation() {
    validate(&format!(
        "{}\n{}",
        include_str!("../src/render/common.wgsl"),
        include_str!("../src/render/sky.wgsl")
    ));
}
#[test]
fn hdr_post_shader_passes_naga_validation() {
    validate(include_str!("../src/render/post.wgsl"));
}
#[test]
fn pixel_gui_shader_passes_naga_validation() {
    validate(include_str!("../src/render/ui.wgsl"));
}
#[test]
fn base_entity_instance_shader_passes_naga_validation_without_a_gpu() {
    validate(&format!(
        "{}\n{}\n{}",
        include_str!("../src/render/common.wgsl"),
        include_str!("../src/render/terrain.wgsl"),
        include_str!("../src/render/entity.wgsl")
    ));
}
