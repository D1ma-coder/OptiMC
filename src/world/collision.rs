//! Collision adapter over server chunks and official state-shape data.
use super::remote::RemoteWorld;
use crate::{assets::collision::Physics, player::CollisionWorld};
use glam::DVec3;
pub struct RemoteCollision<'a> {
    pub world: &'a RemoteWorld,
    pub physics: &'a Physics,
}
impl RemoteCollision<'_> {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<&crate::assets::collision::StatePhysics> {
        self.world
            .block(x, y, z)
            .and_then(|id| self.physics.state(id))
    }
    fn factors(&self, feet: DVec3, jump: bool) -> f32 {
        let factor = |y: f64| {
            self.state(
                feet.x.floor() as i32,
                y.floor() as i32,
                feet.z.floor() as i32,
            )
            .map(|s| if jump { s.jump } else { s.speed })
            .unwrap_or(1.)
        };
        let inside = factor(feet.y);
        if inside != 1. {
            inside
        } else {
            factor(feet.y - 0.500001f32 as f64)
        }
    }
    pub fn loaded(&self, feet: DVec3) -> bool {
        self.world.chunks.contains_key(&(
            (feet.x.floor() as i32).div_euclid(16),
            (feet.z.floor() as i32).div_euclid(16),
        ))
    }
}
impl CollisionWorld for RemoteCollision<'_> {
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        !self
            .world
            .chunks
            .contains_key(&(x.div_euclid(16), z.div_euclid(16)))
            || self.state(x, y, z).is_some_and(|s| !s.boxes.is_empty())
    }
    fn boxes(&self, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(DVec3, DVec3)) {
        if !self
            .world
            .chunks
            .contains_key(&(x.div_euclid(16), z.div_euclid(16)))
        {
            let min = DVec3::new(x as f64, y as f64, z as f64);
            visit(min, min + DVec3::ONE);
            return;
        }
        if let Some(state) = self.state(x, y, z) {
            let origin = DVec3::new(x as f64, y as f64, z as f64);
            for b in state.boxes.iter() {
                visit(
                    origin + DVec3::new(b[0], b[1], b[2]),
                    origin + DVec3::new(b[3], b[4], b[5]),
                );
            }
        }
    }
    fn visual_boxes(&self, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(DVec3, DVec3)) {
        if !self
            .world
            .chunks
            .contains_key(&(x.div_euclid(16), z.div_euclid(16)))
        {
            self.boxes(x, y, z, visit);
            return;
        }
        let Some(id) = self.world.block(x, y, z) else {
            return;
        };
        let visual = crate::assets::collision::visual().unwrap_or(self.physics);
        if let Some(state) = visual.state(id) {
            let origin = DVec3::new(x as f64, y as f64, z as f64);
            for b in state.boxes.iter() {
                visit(
                    origin + DVec3::new(b[0], b[1], b[2]),
                    origin + DVec3::new(b[3], b[4], b[5]),
                );
            }
        }
    }
    fn friction(&self, feet: DVec3) -> f32 {
        self.state(
            feet.x.floor() as i32,
            (feet.y - 0.500001f32 as f64).floor() as i32,
            feet.z.floor() as i32,
        )
        .map(|s| s.friction)
        .unwrap_or(0.6)
    }
    fn speed_factor(&self, feet: DVec3) -> f32 {
        self.factors(feet, false)
    }
    fn jump_factor(&self, feet: DVec3) -> f32 {
        self.factors(feet, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        net::{
            chunk::{Chunk, Section},
            registry::Dimension,
        },
        player::{Input, Player},
    };
    fn world() -> RemoteWorld {
        let mut world = RemoteWorld {
            dimension: Some(Dimension {
                name: "minecraft:overworld".into(),
                min_y: 0,
                height: 16,
                logical_height: 16,
                skylight: true,
                ceiling: false,
            }),
            ..RemoteWorld::default()
        };
        world
            .insert(Chunk {
                block_entities: vec![],
                x: 0,
                z: 0,
                sections: vec![Section {
                    non_air: 0,
                    fluids: 0,
                    blocks: vec![0; 4096],
                    biomes: vec![0; 64],
                }],
            })
            .unwrap();
        let stone = crate::assets::builtins::get()
            .unwrap()
            .default_state("minecraft:stone")
            .unwrap();
        for x in 0..16 {
            for z in 0..16 {
                world.set_block(x, 0, z, stone);
            }
        }
        world
    }
    #[test]
    fn server_slabs_step_by_half_a_block_and_tall_walls_stop_the_body() {
        let mut world = world();
        let ids = crate::assets::builtins::get().unwrap();
        let slab = ids.default_state("minecraft:oak_slab").unwrap();
        let stone = ids.default_state("minecraft:stone").unwrap();
        for x in 9..14 {
            for z in 7..10 {
                world.set_block(x, 1, z, slab);
            }
        }
        for y in 1..5 {
            for z in 7..10 {
                world.set_block(14, y, z, stone);
            }
        }
        let collision = RemoteCollision {
            world: &world,
            physics: crate::assets::collision::get().unwrap(),
        };
        let mut player = Player::spawn(DVec3::new(8.5, 1., 8.5));
        for _ in 0..80 {
            player.tick(
                Input {
                    strafe: 1.,
                    ..Input::default()
                },
                0.,
                &collision,
            );
        }
        assert!((player.feet.x - 13.7).abs() < 1e-6, "{:?}", player.feet);
        assert!((player.feet.y - 1.5).abs() < 1e-6, "{:?}", player.feet);
        assert!(player.grounded && player.horizontal_collision);
    }
    #[test]
    fn fence_shape_above_its_cell_supports_player_and_unloaded_columns_block_exit() {
        let mut world = world();
        let fence = crate::assets::builtins::get()
            .unwrap()
            .default_state("minecraft:oak_fence")
            .unwrap();
        world.set_block(8, 1, 8, fence);
        let collision = RemoteCollision {
            world: &world,
            physics: crate::assets::collision::get().unwrap(),
        };
        let mut player = Player::spawn(DVec3::new(8.5, 4., 8.5));
        for _ in 0..60 {
            player.tick(Input::default(), 0., &collision);
        }
        assert!((player.feet.y - 2.5).abs() < 1e-6, "{:?}", player.feet);
        let mut player = Player::spawn(DVec3::new(15.5, 1., 4.5));
        for _ in 0..40 {
            player.tick(
                Input {
                    strafe: 1.,
                    ..Input::default()
                },
                0.,
                &collision,
            );
        }
        assert!((player.feet.x - 15.7).abs() < 1e-6);
        assert!(collision.loaded(player.feet));
    }
}
