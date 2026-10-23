//! Independent 20 Hz player movement. Dry full-block terrain first; fluid,
//! ladder, status-effect and vehicle rules are tracked separately in STATUS.
use glam::{DVec3, Mat4, Vec3};
pub const TICK: f64 = 1. / 20.;
pub const WIDTH: f64 = 0.6;
pub const STANDING_HEIGHT: f64 = 1.8;
pub const EYE_HEIGHT: f64 = 1.62;
pub trait CollisionWorld {
    fn solid(&self, x: i32, y: i32, z: i32) -> bool;
    fn boxes(&self, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(DVec3, DVec3)) {
        if self.solid(x, y, z) {
            let min = DVec3::new(x as f64, y as f64, z as f64);
            visit(min, min + DVec3::ONE);
        }
    }
    fn visual_boxes(&self, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(DVec3, DVec3)) {
        self.boxes(x, y, z, visit);
    }
    fn friction(&self, _feet: DVec3) -> f32 {
        0.6
    }
    fn speed_factor(&self, _feet: DVec3) -> f32 {
        1.
    }
    fn jump_factor(&self, _feet: DVec3) -> f32 {
        1.
    }
}
pub struct DemoCollision;
impl CollisionWorld for DemoCollision {
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        y >= 0 && y < crate::world::height(x, z)
    }
}
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub forward: f64,
    pub strafe: f64,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Perspective {
    #[default]
    First,
    Back,
    Front,
}
impl Perspective {
    pub fn cycle(self) -> Self {
        match self {
            Self::First => Self::Back,
            Self::Back => Self::Front,
            Self::Front => Self::First,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Player {
    pub feet: DVec3,
    pub previous: DVec3,
    pub velocity: DVec3,
    pub grounded: bool,
    pub horizontal_collision: bool,
    pub crouching: bool,
    pub perspective: Perspective,
    pub walk_distance: f32,
    pub previous_walk: f32,
    pub bob: f32,
    pub previous_bob: f32,
    pub sprinting: bool,
    pub fov_multiplier: f32,
    pub previous_fov: f32,
    pub abilities: crate::net::abilities::Abilities,
    pub attributes: crate::net::attributes::Movement,
    pub no_clip: bool,
    pub food: i32,
    eye_height: f64,
    previous_eye_height: f64,
    accumulator: f64,
    jump_delay: u8,
    flight_toggle: u8,
    last_jump: bool,
}
#[derive(Clone, Copy)]
struct Aabb {
    min: DVec3,
    max: DVec3,
}
impl Aabb {
    fn player(feet: DVec3, height: f64) -> Self {
        Self {
            min: feet - DVec3::new(WIDTH * 0.5, 0., WIDTH * 0.5),
            max: feet + DVec3::new(WIDTH * 0.5, height, WIDTH * 0.5),
        }
    }
    fn shifted(self, v: DVec3) -> Self {
        Self {
            min: self.min + v,
            max: self.max + v,
        }
    }
    fn collides(self, world: &impl CollisionWorld) -> bool {
        let mut hit = false;
        for y in self.min.y.floor() as i32 - 1..=(self.max.y - 1e-8).floor() as i32 + 1 {
            for z in self.min.z.floor() as i32 - 1..=(self.max.z - 1e-8).floor() as i32 + 1 {
                for x in self.min.x.floor() as i32 - 1..=(self.max.x - 1e-8).floor() as i32 + 1 {
                    world.boxes(x, y, z, &mut |min, max| {
                        hit |= (0..3)
                            .all(|i| self.max[i] > min[i] + 1e-8 && self.min[i] < max[i] - 1e-8);
                    });
                    if hit {
                        return true;
                    }
                }
            }
        }
        hit
    }
    fn clip(self, axis: usize, delta: f64, world: &impl CollisionWorld) -> f64 {
        if delta == 0. {
            return 0.;
        }
        let mut low = self.min;
        let mut high = self.max;
        low[axis] += delta.min(0.);
        high[axis] += delta.max(0.);
        let mut allowed = delta;
        for y in low.y.floor() as i32 - 1..=(high.y - 1e-8).floor() as i32 + 1 {
            for z in low.z.floor() as i32 - 1..=(high.z - 1e-8).floor() as i32 + 1 {
                for x in low.x.floor() as i32 - 1..=(high.x - 1e-8).floor() as i32 + 1 {
                    world.boxes(x, y, z, &mut |bmin, bmax| {
                        if (0..3)
                            .filter(|&i| i != axis)
                            .any(|i| self.max[i] <= bmin[i] + 1e-8 || self.min[i] >= bmax[i] - 1e-8)
                        {
                            return;
                        }
                        if delta > 0. && self.max[axis] <= bmin[axis] + 1e-8 {
                            allowed = allowed.min((bmin[axis] - self.max[axis]).max(0.));
                        }
                        if delta < 0. && self.min[axis] >= bmax[axis] - 1e-8 {
                            allowed = allowed.max((bmax[axis] - self.min[axis]).min(0.));
                        }
                    });
                }
            }
        }
        allowed
    }
}
fn collide(bounds: Aabb, movement: DVec3, world: &impl CollisionWorld) -> DVec3 {
    let mut result = DVec3::ZERO;
    let mut bounds = bounds;
    result.y = bounds.clip(1, movement.y, world);
    bounds = bounds.shifted(DVec3::Y * result.y);
    let axes = if movement.x.abs() < movement.z.abs() {
        [2, 0]
    } else {
        [0, 2]
    };
    for axis in axes {
        result[axis] = bounds.clip(axis, movement[axis], world);
        let mut delta = DVec3::ZERO;
        delta[axis] = result[axis];
        bounds = bounds.shifted(delta);
    }
    result
}
impl Player {
    pub fn spawn(feet: DVec3) -> Self {
        Self {
            feet,
            previous: feet,
            velocity: DVec3::ZERO,
            grounded: false,
            horizontal_collision: false,
            crouching: false,
            perspective: Perspective::First,
            walk_distance: 0.,
            previous_walk: 0.,
            bob: 0.,
            previous_bob: 0.,
            sprinting: false,
            fov_multiplier: 1.,
            previous_fov: 1.,
            abilities: crate::net::abilities::Abilities::default(),
            attributes: Default::default(),
            no_clip: false,
            food: 20,
            eye_height: EYE_HEIGHT,
            previous_eye_height: EYE_HEIGHT,
            accumulator: 0.,
            jump_delay: 0,
            flight_toggle: 0,
            last_jump: false,
        }
    }
    pub fn advance(&mut self, seconds: f64, input: Input, yaw: f32, world: &impl CollisionWorld) {
        self.advance_with(seconds, input, yaw, world, |_| {});
    }
    pub fn advance_with(
        &mut self,
        seconds: f64,
        input: Input,
        yaw: f32,
        world: &impl CollisionWorld,
        mut after_tick: impl FnMut(&Self),
    ) {
        self.accumulator += seconds.clamp(0., 0.25);
        while self.accumulator + 1e-12 >= TICK {
            self.tick(input, yaw, world);
            after_tick(self);
            self.accumulator = (self.accumulator - TICK).max(0.);
        }
    }
    pub fn correct(&mut self, pose: crate::net::entity::Pose) {
        self.feet = pose.position;
        self.previous = pose.position;
        self.velocity = pose.velocity;
        self.grounded = false;
        self.horizontal_collision = false;
        // The game tick clock is independent of server pose corrections.
    }
    pub fn alpha(&self) -> f32 {
        (self.accumulator / TICK) as f32
    }
    pub fn interpolated_feet(&self) -> DVec3 {
        self.previous.lerp(self.feet, self.accumulator / TICK)
    }
    pub fn eye(&self) -> Vec3 {
        self.eye_precise().as_vec3()
    }
    pub fn eye_precise(&self) -> DVec3 {
        self.interpolated_feet()
            + DVec3::Y
                * (self.previous_eye_height
                    + (self.eye_height - self.previous_eye_height) * self.alpha() as f64)
    }
    pub fn tick(&mut self, input: Input, yaw: f32, world: &impl CollisionWorld) {
        self.previous = self.feet;
        self.previous_walk = self.walk_distance;
        self.previous_bob = self.bob;
        self.previous_fov = self.fov_multiplier;
        // LivingEntity.aiStep trims existing motion before acceleration/travel.
        if self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z < 9e-6 {
            self.velocity.x = 0.;
            self.velocity.z = 0.;
        }
        if self.velocity.y.abs() < 0.003 {
            self.velocity.y = 0.;
        }
        self.jump_delay = self.jump_delay.saturating_sub(1);
        self.flight_toggle = self.flight_toggle.saturating_sub(1);
        if self.abilities.may_fly && input.jump && !self.last_jump {
            if self.flight_toggle > 0 {
                self.abilities.flying = !self.abilities.flying;
                self.flight_toggle = 0;
            } else {
                self.flight_toggle = 7;
            }
        }
        self.last_jump = input.jump;
        if self.no_clip {
            self.abilities.flying = true;
        }
        let flying = self.abilities.flying;
        let wants_crouch = input.sneak && !flying;
        self.crouching = wants_crouch
            || (self.crouching && Aabb::player(self.feet, STANDING_HEIGHT).collides(world));
        self.previous_eye_height = self.eye_height;
        self.eye_height +=
            ((if self.crouching { 1.27 } else { EYE_HEIGHT }) - self.eye_height) * 0.5;
        self.sprinting = input.sprint
            && input.forward > 0.
            && !self.crouching
            && (self.food > 6 || self.abilities.may_fly);
        self.fov_multiplier += (if self.sprinting { 1.15 } else { 1. } - self.fov_multiplier) * 0.5;
        let direction = DVec3::new((yaw as f64).sin(), 0., -(yaw as f64).cos());
        let right = direction.cross(DVec3::Y);
        let mut movement =
            (direction * input.forward + right * input.strafe).clamp_length_max(1.) * 0.98;
        if self.crouching {
            movement *= 0.3;
        }
        if input.jump && self.grounded && self.jump_delay == 0 && !flying {
            self.velocity.y = (self.attributes.jump as f32 * world.jump_factor(self.feet)) as f64;
            if self.sprinting {
                self.velocity += direction * 0.2;
            }
            self.jump_delay = 10;
        }
        if !input.jump {
            self.jump_delay = 0;
        }
        let was_grounded = self.grounded;
        let modified_friction =
            |base: f32, modifier: f64| (1. - (1. - base) * modifier as f32).clamp(0., 1.);
        let friction = modified_friction(world.friction(self.feet), self.attributes.friction);
        let accel = if flying {
            (self.abilities.flying_speed * if self.sprinting { 2. } else { 1. }) as f64
        } else if self.grounded {
            let speed = (self.attributes.speed * if self.sprinting { 1.3 } else { 1. }) as f32;
            (speed
                * if friction as f64 > 0.6 {
                    0.21600002 / (friction * friction * friction)
                } else {
                    1.
                }) as f64
        } else if self.sprinting {
            0.026f32 as f64
        } else {
            0.02f32 as f64
        };
        self.velocity += movement * accel;
        if flying {
            self.velocity.y += ((input.jump as i32 - input.sneak as i32) as f32
                * self.abilities.flying_speed
                * 3.) as f64;
        }
        let flying_y = self.velocity.y;
        let height = if self.crouching { 1.5 } else { STANDING_HEIGHT };
        let bounds = Aabb::player(self.feet, height);
        let mut desired = self.velocity;
        if self.crouching && self.grounded {
            for axis in [0, 2] {
                while desired[axis].abs() > 1e-8 {
                    let mut offset = DVec3::new(0., -0.6, 0.);
                    offset[axis] = desired[axis];
                    if bounds.shifted(offset).collides(world) {
                        break;
                    }
                    desired[axis] = if desired[axis].abs() <= 0.05 {
                        0.
                    } else {
                        desired[axis] - desired[axis].signum() * 0.05
                    };
                }
            }
            while desired.x.abs() > 1e-8
                && desired.z.abs() > 1e-8
                && !bounds
                    .shifted(DVec3::new(desired.x, -0.6, desired.z))
                    .collides(world)
            {
                for axis in [0, 2] {
                    desired[axis] = if desired[axis].abs() <= 0.05 {
                        0.
                    } else {
                        desired[axis] - desired[axis].signum() * 0.05
                    };
                }
            }
        }
        let mut resolved = if self.no_clip {
            desired
        } else {
            collide(bounds, desired, world)
        };
        if !self.no_clip
            && (was_grounded || desired.y < 0. && resolved.y != desired.y)
            && (resolved.x != desired.x || resolved.z != desired.z)
        {
            let up = collide(bounds, DVec3::Y * self.attributes.step, world);
            let across = collide(
                bounds.shifted(up),
                DVec3::new(desired.x, 0., desired.z),
                world,
            );
            let down = collide(
                bounds.shifted(up + across),
                DVec3::Y * (-up.y + desired.y),
                world,
            );
            if across.x * across.x + across.z * across.z
                > resolved.x * resolved.x + resolved.z * resolved.z
            {
                resolved = up + across + down;
            }
        }
        self.horizontal_collision =
            (resolved.x - desired.x).abs() > 1e-8 || (resolved.z - desired.z).abs() > 1e-8;
        self.feet += resolved;
        self.grounded = desired.y < 0. && (resolved.y - desired.y).abs() > 1e-8;
        for axis in 0..3 {
            if (resolved[axis] - desired[axis]).abs() > 1e-8 {
                self.velocity[axis] = 0.;
            }
        }
        self.velocity.x *= if was_grounded {
            (friction * modified_friction(0.91f32, self.attributes.air_drag)) as f64
        } else {
            modified_friction(0.91f32, self.attributes.air_drag) as f64
        };
        self.velocity.z *= if was_grounded {
            (friction * modified_friction(0.91f32, self.attributes.air_drag)) as f64
        } else {
            modified_friction(0.91f32, self.attributes.air_drag) as f64
        };
        let speed_factor = world.speed_factor(self.feet) as f64;
        self.velocity.x *= speed_factor;
        self.velocity.z *= speed_factor;
        self.velocity.y = if flying {
            flying_y * 0.6
        } else {
            (self.velocity.y - self.attributes.gravity)
                * modified_friction(0.98f32, self.attributes.air_drag) as f64
        };
        if self.grounded && flying && !self.no_clip {
            self.abilities.flying = false;
        }
        let horizontal = resolved.x.hypot(resolved.z);
        self.walk_distance += horizontal as f32 * 0.6;
        let target = if self.grounded {
            horizontal.min(0.1) as f32
        } else {
            0.
        };
        self.bob += (target - self.bob) * 0.4;
    }
    /// GameRenderer.bobView's translation and roll/pitch amplitudes from 26.2.
    pub fn bob_matrix(&self) -> Mat4 {
        self.bob_matrix_scaled(1.)
    }
    pub fn bob_matrix_scaled(&self, intensity: f32) -> Mat4 {
        let phase = -(self.walk_distance
            + (self.walk_distance - self.previous_walk) * self.alpha())
            * std::f32::consts::PI;
        let bob = (self.previous_bob + (self.bob - self.previous_bob) * self.alpha()) * intensity;
        Mat4::from_translation(Vec3::new(
            phase.sin() * bob * 0.5,
            -(phase.cos() * bob).abs(),
            0.,
        )) * Mat4::from_rotation_z((phase.sin() * bob * 3.).to_radians())
            * Mat4::from_rotation_x(
                ((phase - 0.2).cos() * bob)
                    .abs()
                    .mul_add(5., 0.)
                    .to_radians(),
            )
    }
    pub fn camera(&self, yaw: f32, pitch: f32, world: &impl CollisionWorld) -> (Vec3, Vec3) {
        let eye = self.eye();
        let forward = Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        );
        if self.perspective == Perspective::First {
            return (eye, forward);
        }
        let sign = if self.perspective == Perspective::Back {
            -1.
        } else {
            1.
        };
        let mut distance = 4f32;
        // Camera collision samples the eight corners of its small collision box.
        for corner in 0..8 {
            let offset = Vec3::new(
                if corner & 1 == 0 { -0.1 } else { 0.1 },
                if corner & 2 == 0 { -0.1 } else { 0.1 },
                if corner & 4 == 0 { -0.1 } else { 0.1 },
            );
            let origin = self.interpolated_feet()
                + DVec3::Y
                    * (self.previous_eye_height
                        + (self.eye_height - self.previous_eye_height) * self.alpha() as f64);
            let from = origin + offset.as_dvec3();
            let to = from + forward.as_dvec3() * (distance * sign) as f64;
            if let Some(hit) = crate::world::raycast::visual(world, from, to) {
                let squared = hit.position.distance_squared(origin) as f32;
                if squared < distance * distance {
                    distance = squared.sqrt();
                }
            }
        }
        (
            eye + forward * (distance * sign),
            forward * if sign > 0. { -1. } else { 1. },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_corrections_do_not_reset_the_global_physics_tick_phase() {
        let mut p = Player::spawn(DVec3::new(0., 80., 0.));
        let mut ticks = 0;
        for _ in 0..100 {
            p.advance_with(0.01, Input::default(), 0., &Flat, |_| ticks += 1);
            p.correct(crate::net::entity::Pose {
                position: p.feet,
                velocity: p.velocity,
                ..Default::default()
            });
        }
        assert_eq!(ticks, 20);
    }
    #[test]
    fn falling_trajectory_matches_original_java_float_drag_for_120_ticks() {
        struct Empty;
        impl CollisionWorld for Empty {
            fn solid(&self, _x: i32, _y: i32, _z: i32) -> bool {
                false
            }
        }
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../data/26.2/physics-golden.json")).unwrap();
        let mut p = Player::spawn(DVec3::new(0., 80., 0.));
        for frame in golden["empty_world_fall"].as_array().unwrap() {
            p.tick(Input::default(), 0., &Empty);
            assert_eq!(
                p.feet.y,
                f64::from_bits(u64::from_str_radix(frame["y_bits"].as_str().unwrap(), 16).unwrap()),
                "tick {}",
                frame["tick"]
            );
            assert_eq!(
                p.velocity.y,
                f64::from_bits(
                    u64::from_str_radix(frame["vy_bits"].as_str().unwrap(), 16).unwrap()
                ),
                "tick {}",
                frame["tick"]
            );
        }
        p.velocity = DVec3::new(0.0029, 0.0029, 0.0029);
        let before = p.feet;
        p.tick(Input::default(), 0., &Empty);
        assert_eq!(p.feet.y, before.y);
        assert!(
            p.feet.x > before.x && p.feet.z > before.z,
            "Player horizontal threshold is on squared vector length"
        );
    }
    #[test]
    fn flight_respects_server_permission_and_vertical_speed_without_gravity() {
        let mut player = Player::spawn(DVec3::new(0., 10., 0.));
        player.abilities = crate::net::abilities::Abilities {
            may_fly: true,
            flying: true,
            ..Default::default()
        };
        for _ in 0..10 {
            player.tick(Input::default(), 0., &Flat);
        }
        assert_eq!(player.feet.y, 10.);
        player.tick(
            Input {
                jump: true,
                ..Input::default()
            },
            0.,
            &Flat,
        );
        assert!((player.feet.y - 10. - 0.15).abs() < 1e-7);
        player.tick(Input::default(), 0., &Flat);
        player.tick(
            Input {
                jump: true,
                ..Input::default()
            },
            0.,
            &Flat,
        );
        assert!(
            !player.abilities.flying,
            "Double jump toggles permitted flight"
        );
        player.abilities = Default::default();
        for jump in [false, true, false, true] {
            player.tick(
                Input {
                    jump,
                    ..Input::default()
                },
                0.,
                &Flat,
            );
        }
        assert!(!player.abilities.flying, "Survival cannot enable flight");
    }
    #[test]
    fn crouching_checks_combined_diagonal_support_at_inside_corners() {
        struct Corner;
        impl CollisionWorld for Corner {
            fn solid(&self, x: i32, y: i32, z: i32) -> bool {
                y < 0 && (x < 1 || z < 1)
            }
        }
        let mut player = Player::spawn(DVec3::new(0.95, 0., 0.95));
        player.tick(Input::default(), 0., &Corner);
        player.tick(Input::default(), 0., &Corner);
        for _ in 0..100 {
            player.tick(
                Input {
                    forward: 1.,
                    strafe: -1.,
                    sneak: true,
                    ..Input::default()
                },
                std::f32::consts::PI,
                &Corner,
            );
            assert!(
                player.feet.y >= -1e-8,
                "Sneaking fell through diagonal edge: {:?}",
                player.feet
            );
        }
    }
    struct Flat;
    impl CollisionWorld for Flat {
        fn solid(&self, _: i32, y: i32, _: i32) -> bool {
            y < 0
        }
    }
    fn standing() -> Player {
        let mut p = Player::spawn(DVec3::ZERO);
        p.tick(Input::default(), 0., &Flat);
        p.tick(Input::default(), 0., &Flat);
        p
    }
    #[test]
    fn jumping_has_vanilla_dry_ground_height_and_lands() {
        let mut p = standing();
        let mut peak = 0f64;
        for tick in 0..100 {
            p.tick(
                Input {
                    jump: tick == 0,
                    ..Input::default()
                },
                0.,
                &Flat,
            );
            peak = peak.max(p.feet.y);
        }
        assert!((peak - 1.2522).abs() < 0.002, "Jump height {peak}");
        assert!(p.feet.y.abs() < 1e-8);
        assert!(p.grounded);
    }
    #[test]
    fn frame_rate_does_not_change_simulated_movement() {
        let mut a = standing();
        let mut b = a.clone();
        let input = Input {
            forward: 1.,
            ..Input::default()
        };
        for _ in 0..400 {
            a.advance(0.05, input, 0., &Flat);
        }
        for _ in 0..4000 {
            b.advance(0.005, input, 0., &Flat);
        }
        assert!(a.feet.distance(b.feet) < 1e-8);
        let speed = -(a.feet.z - a.previous.z) * 20.;
        assert!((speed - 4.317).abs() < 0.015, "Walk speed {speed}");
    }
    #[test]
    fn walls_stop_body_and_head_does_not_cross_ceiling() {
        struct Room;
        impl CollisionWorld for Room {
            fn solid(&self, x: i32, y: i32, _: i32) -> bool {
                y < 0 || x >= 2 || y >= 3
            }
        }
        let mut p = standing();
        for _ in 0..80 {
            p.tick(
                Input {
                    strafe: 1.,
                    jump: true,
                    ..Input::default()
                },
                0.,
                &Room,
            );
            assert!(p.feet.x <= 1.7 + 1e-7);
            assert!(p.feet.y + 1.8 <= 3. + 1e-7);
        }
    }
    #[test]
    fn third_person_camera_stops_at_wall() {
        struct Wall;
        impl CollisionWorld for Wall {
            fn solid(&self, _: i32, _: i32, z: i32) -> bool {
                z >= 2
            }
        }
        let mut p = standing();
        p.perspective = Perspective::Back;
        let (camera, _) = p.camera(0., 0., &Wall);
        // Golden from original BlockGetter.clip(VISUAL/NONE) + Camera.getMaxZoom:
        // hit points are measured from the eye centre, including ±.1 corner offsets.
        assert!((camera.z - 2.0049937).abs() < 0.00001, "{camera:?}");
    }
}
