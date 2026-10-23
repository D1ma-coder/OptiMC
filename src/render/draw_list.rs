//! CPU draw lists retain capacity across frames; no device or GPU readback.
use crate::world::lod::Frustum;
use glam::Vec3;
use std::collections::HashSet;
#[derive(Default)]
pub struct DrawList {
    pub opaque: Vec<usize>,
    pub transparent: Vec<(usize, f32)>,
}
impl DrawList {
    pub fn update(
        &mut self,
        meshes: impl IntoIterator<Item = (bool, Vec3, Vec3, Option<[i32; 3]>)>,
        frustum: &Frustum,
        visible: Option<&HashSet<[i32; 3]>>,
        eye: Vec3,
        enabled: bool,
    ) {
        self.opaque.clear();
        self.transparent.clear();
        if !enabled {
            return;
        }
        for (index, (translucent, min, max, section)) in meshes.into_iter().enumerate() {
            if visible.is_some_and(|set| section.is_some_and(|s| !set.contains(&s)))
                || !frustum.visible(min, max)
            {
                continue;
            }
            if translucent {
                self.transparent
                    .push((index, ((min + max) * 0.5 - eye).length_squared()));
            } else {
                self.opaque.push(index);
            }
        }
        // Explicit tie-break preserves source order without a stable sort's
        // auxiliary allocation. Distance is calculated once, not per comparison.
        self.transparent
            .sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn culling_order_ties_and_reuse_preserve_reference_draws() {
        let frustum = Frustum::new(glam::Mat4::IDENTITY);
        let meshes = [
            (false, Vec3::splat(-0.1), Vec3::splat(0.1), None),
            (true, Vec3::splat(0.1), Vec3::splat(0.2), None),
            (true, Vec3::splat(0.5), Vec3::splat(0.6), None),
            (true, Vec3::splat(0.5), Vec3::splat(0.6), None),
            (false, Vec3::splat(5.), Vec3::splat(6.), None),
            (false, Vec3::ZERO, Vec3::splat(0.1), Some([1, 2, 3])),
        ];
        let mut list = DrawList::default();
        let visible = HashSet::new();
        list.update(meshes, &frustum, Some(&visible), Vec3::ZERO, true);
        assert_eq!(list.opaque, [0]);
        assert_eq!(
            list.transparent.iter().map(|v| v.0).collect::<Vec<_>>(),
            [2, 3, 1]
        );
        let capacities = (list.opaque.capacity(), list.transparent.capacity());
        list.update(meshes, &frustum, Some(&visible), Vec3::ZERO, true);
        assert_eq!(
            capacities,
            (list.opaque.capacity(), list.transparent.capacity())
        );
        list.update(meshes, &frustum, None, Vec3::ZERO, false);
        assert!(list.opaque.is_empty() && list.transparent.is_empty());
        list.update(meshes, &frustum, None, Vec3::ZERO, true);
        assert_eq!(list.opaque, [0, 5]);
    }
}
