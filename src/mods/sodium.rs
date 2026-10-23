// SPDX-License-Identifier: PolyForm-Shield-1.0.0
// Rust behavioral port of Sodium 0.9.2 directional section visibility.
// Upstream: CaffeineMC/Sodium. See LICENSES/Sodium.txt and THIRD_PARTY_NOTICES.md.
use std::collections::{HashMap, HashSet, VecDeque};
pub const ALL: u64 = (1u64 << 36) - 1;
const STEPS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
const AXIS: [usize; 6] = [1, 1, 2, 2, 0, 0];
const MONOTONIC: [u8; 4] = [21, 22, 25, 37];
fn test(bits: &[u64; 64], index: usize) -> bool {
    bits[index / 64] & (1 << (index % 64)) != 0
}
fn set(bits: &mut [u64; 64], index: usize) {
    bits[index / 64] |= 1 << (index % 64);
}
fn index([x, y, z]: [i32; 3]) -> usize {
    (x + z * 16 + y * 256) as usize
}
/// Four monotonic direction sets, shared empty/full fast paths, and exact
/// boundary-to-boundary visibility. Opaque input must mean a full occluder.
pub fn resolve(opaque: &[u64; 64]) -> Vec<u64> {
    let filled: u32 = opaque.iter().map(|v| v.count_ones()).sum();
    if filled == 4096 {
        return vec![0];
    }
    if filled < 256 {
        return vec![ALL];
    }
    MONOTONIC
        .into_iter()
        .map(|allowed| {
            let mut matrix = 0u64;
            for entry in 0..6 {
                if allowed & (1 << entry) != 0 {
                    continue;
                }
                let mut visited = *opaque;
                let mut stack = Vec::with_capacity(64);
                for x in 0..16 {
                    for y in 0..16 {
                        for z in 0..16 {
                            let position = [x, y, z];
                            if position[AXIS[entry]] != if entry % 2 == 0 { 0 } else { 15 } {
                                continue;
                            }
                            let start = index(position);
                            if test(&visited, start) {
                                continue;
                            }
                            set(&mut visited, start);
                            stack.push(position);
                            let mut exits = 0u8;
                            while let Some(p) = stack.pop() {
                                for (direction, step) in STEPS.iter().enumerate() {
                                    if allowed & (1 << direction) == 0 {
                                        continue;
                                    }
                                    let next = std::array::from_fn(|i| p[i] + step[i]);
                                    if next[AXIS[direction]] < 0 || next[AXIS[direction]] >= 16 {
                                        exits |= 1 << direction;
                                        continue;
                                    }
                                    let i = index(next);
                                    if !test(&visited, i) {
                                        set(&mut visited, i);
                                        stack.push(next);
                                    }
                                }
                            }
                            matrix |= 1 << (entry * 6 + entry);
                            for exit in 0..6 {
                                if exits & (1 << exit) != 0 {
                                    matrix |= 1 << (entry * 6 + exit);
                                    matrix |= 1 << (exit * 6 + entry);
                                }
                            }
                        }
                    }
                }
            }
            matrix
        })
        .collect()
}
#[derive(Default)]
pub struct Graph {
    pub sections: HashMap<[i32; 3], u64>,
}
impl Graph {
    /// Conservative traversal: missing sections inside the loaded bounding box
    /// remain open. An unbounded/disconnected-world case disables culling.
    pub fn visible(&self, root: [i32; 3]) -> Option<HashSet<[i32; 3]>> {
        if self.sections.is_empty() {
            return None;
        }
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for p in self.sections.keys() {
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        let mut volume = 1u64;
        for axis in 0..3 {
            if root[axis] < min[axis] || root[axis] > max[axis] {
                return None;
            }
            volume = volume.checked_mul((max[axis] as i64 - min[axis] as i64 + 1) as u64)?;
        }
        if volume > 262144 {
            return None;
        }
        let mut reached = HashMap::<[i32; 3], u8>::new();
        let mut visible = HashSet::new();
        let mut queue = VecDeque::from([(root, 6usize)]);
        reached.insert(root, 1 << 6);
        while let Some((position, entry)) = queue.pop_front() {
            visible.insert(position);
            let visibility = self.sections.get(&position).copied().unwrap_or(ALL);
            for (exit, step) in STEPS.iter().enumerate() {
                let axis = AXIS[exit];
                if (position[axis] > root[axis] && step[axis] < 0)
                    || (position[axis] < root[axis] && step[axis] > 0)
                {
                    continue;
                }
                if entry != 6 && visibility & (1 << (entry * 6 + exit)) == 0 {
                    continue;
                }
                let next = std::array::from_fn(|i| position[i] + step[i]);
                if (0..3).any(|i| next[i] < min[i] || next[i] > max[i]) {
                    continue;
                }
                let next_entry = exit ^ 1;
                let entries = reached.entry(next).or_default();
                if *entries & (1 << next_entry) == 0 {
                    *entries |= 1 << next_entry;
                    queue.push_back((next, next_entry));
                }
            }
        }
        Some(visible)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn original_sodium_directional_visibility_matches_empty_solid_planes_and_random_sections() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/mods/sodium-visibility-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let rows: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for (case, row) in rows.as_array().unwrap().iter().enumerate() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["opaque"].as_str().unwrap())
                .unwrap();
            let opaque = std::array::from_fn(|i| {
                u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
            });
            let expected: Vec<u64> = row["matrices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            assert_eq!(resolve(&opaque), expected, "case {case}");
        }
    }
    #[test]
    fn occlusion_keeps_wall_faces_and_opens_missing_sections_without_hiding_far_world() {
        let mut graph = Graph::default();
        for x in 0..4 {
            graph
                .sections
                .insert([x, 0, 0], if x == 1 { 0 } else { ALL });
        }
        let visible = graph.visible([0, 0, 0]).unwrap();
        assert!(visible.contains(&[1, 0, 0]));
        assert!(!visible.contains(&[2, 0, 0]));
        graph.sections.remove(&[1, 0, 0]);
        assert!(graph.visible([0, 0, 0]).unwrap().contains(&[3, 0, 0]));
        assert!(graph.visible([100, 0, 0]).is_none());
    }
}

#[derive(Default)]
pub struct Cache {
    graph: Option<std::sync::Arc<Graph>>,
    root: [i32; 3],
    visible: Option<std::sync::Arc<HashSet<[i32; 3]>>>,
}
impl Cache {
    pub fn update(
        &mut self,
        graph: Option<&std::sync::Arc<Graph>>,
        eye: [f32; 3],
    ) -> Option<std::sync::Arc<HashSet<[i32; 3]>>> {
        let graph = graph?;
        let root = eye.map(|v| (v / 16.).floor() as i32);
        if self
            .graph
            .as_ref()
            .is_none_or(|old| !std::sync::Arc::ptr_eq(old, graph))
            || self.root != root
        {
            self.visible = graph.visible(root).map(std::sync::Arc::new);
            self.graph = Some(graph.clone());
            self.root = root;
        }
        self.visible.clone()
    }
}

#[cfg(test)]
mod traversal_tests {
    use super::*;
    #[test]
    fn graph_does_not_bend_view_rays_around_walls_to_reenter_hidden_sections() {
        let mut graph = Graph::default();
        for x in 0..4 {
            for y in 0..2 {
                graph
                    .sections
                    .insert([x, y, 0], if x == 1 && y == 0 { 0 } else { ALL });
            }
        }
        let visible = graph.visible([0, 0, 0]).unwrap();
        assert!(visible.contains(&[1, 0, 0]));
        assert!(!visible.contains(&[3, 0, 0]));
        assert!(visible.contains(&[3, 1, 0]));
    }
}
