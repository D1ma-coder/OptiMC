// SPDX-License-Identifier: LGPL-3.0-or-later
// Behavioral port of Zoomify 2.17.0 (isXander), from the user-supplied JAR.
// Reference: https://github.com/isXander/Zoomify. See THIRD_PARTY_NOTICES.md.
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    Instant,
    Linear,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInExp,
    #[default]
    EaseOutExp,
    EaseInOutExp,
}
impl Transition {
    pub fn opposite(self) -> Self {
        match self {
            Self::EaseInSine => Self::EaseOutSine,
            Self::EaseOutSine => Self::EaseInSine,
            Self::EaseInQuad => Self::EaseOutQuad,
            Self::EaseOutQuad => Self::EaseInQuad,
            Self::EaseInCubic => Self::EaseOutCubic,
            Self::EaseOutCubic => Self::EaseInCubic,
            Self::EaseInExp => Self::EaseOutExp,
            Self::EaseOutExp => Self::EaseInExp,
            _ => self,
        }
    }
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Self::Instant | Self::Linear => t,
            Self::EaseInSine => 1. - (t * PI / 2.).cos(),
            Self::EaseOutSine => (t * PI / 2.).sin(),
            Self::EaseInOutSine => -((PI * t).cos() - 1.) / 2.,
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => 1. - (1. - t) * (1. - t),
            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2. * t * t
                } else {
                    1. - (-2. * t + 2.).powf(2.) / 2.
                }
            }
            Self::EaseInCubic => t.powf(3.),
            Self::EaseOutCubic => 1. - (1. - t).powf(3.),
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4. * t * t * t
                } else {
                    1. - (-2. * t + 2.).powf(3.) / 2.
                }
            }
            // Preserve the downloaded version's exact exp equations, including
            // its 1023 normalization and asymmetric in-out branch.
            Self::EaseInExp | Self::EaseOutExp | Self::EaseInOutExp => {
                if t == 0. || t == 1. {
                    return t;
                }
                let c = 1023f64.log2();
                match self {
                    Self::EaseInExp => 2f64.powf(10. * t - c),
                    Self::EaseOutExp => 1. - 2f64.powf(10. - c - 10. * t),
                    _ => {
                        if t < 0.5 {
                            2f64.powf(20. * t - c)
                        } else {
                            1. - 2f64.powf(10. - c - 10. * t)
                        }
                    }
                }
            }
        }
    }
    pub fn inverse(self, t: f64) -> Option<f64> {
        Some(match self {
            Self::EaseInSine => (1. - t).acos() * 2. / PI,
            Self::EaseOutSine => t.asin() * 2. / PI,
            Self::EaseInQuad => t.sqrt(),
            Self::EaseOutQuad => 1. - (1. - t).sqrt(),
            Self::EaseInCubic => t.powf(1. / 3.),
            Self::EaseOutCubic => 1. - (1. - t).powf(1. / 3.),
            Self::EaseInExp => {
                if t == 0. || t == 1. {
                    t
                } else {
                    (1023. * t + 1.).ln() / (10. * 2f64.ln())
                }
            }
            Self::EaseOutExp => {
                if t == 0. || t == 1. {
                    t
                } else {
                    -((-(1023. * t - 1024.) / 1023.).ln() - 10. * 2f64.ln() + 1023f64.ln())
                        / (10. * 2f64.ln())
                }
            }
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyBehavior {
    #[default]
    Hold,
    Toggle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub enabled: bool,
    pub initial_zoom: u32,
    pub zoom_in_time: f64,
    pub zoom_out_time: f64,
    pub zoom_in_transition: Transition,
    pub zoom_out_transition: Transition,
    pub affect_hand_fov: bool,
    pub retain_zoom_steps: bool,
    pub scroll_zoom: bool,
    pub scroll_step_count: i32,
    pub zoom_per_step: u32,
    pub scroll_zoom_smoothness: u32,
    pub zoom_key_behaviour: KeyBehavior,
    pub relative_sensitivity: u32,
    pub relative_view_bobbing: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_zoom: 4,
            zoom_in_time: 1.,
            zoom_out_time: 0.5,
            zoom_in_transition: Transition::EaseOutExp,
            zoom_out_transition: Transition::EaseOutExp,
            affect_hand_fov: true,
            retain_zoom_steps: false,
            scroll_zoom: true,
            scroll_step_count: 10,
            zoom_per_step: 150,
            scroll_zoom_smoothness: 70,
            zoom_key_behaviour: KeyBehavior::Hold,
            relative_sensitivity: 100,
            relative_view_bobbing: true,
        }
    }
}
impl Config {
    pub fn normalize(&mut self) {
        self.initial_zoom = self.initial_zoom.clamp(1, 500);
        self.zoom_per_step = self.zoom_per_step.clamp(100, 1000);
        self.scroll_step_count = self.scroll_step_count.clamp(0, 100);
        self.scroll_zoom_smoothness = self.scroll_zoom_smoothness.clamp(1, 100);
        self.relative_sensitivity = self.relative_sensitivity.min(100);
        for time in [&mut self.zoom_in_time, &mut self.zoom_out_time] {
            *time = if time.is_finite() {
                time.clamp(0., 30.)
            } else {
                0.5
            };
        }
    }
    pub fn min_tier(&self) -> i32 {
        let base = self.zoom_per_step as f64 / 100.;
        if base <= 1. {
            0
        } else {
            -((self.initial_zoom as f64).log(base).ceil() as i32).clamp(0, self.scroll_step_count)
        }
    }
}
#[derive(Clone, Debug)]
struct Interpolation {
    current: f64,
    previous: f64,
    previous_target: f64,
    active: Transition,
    inactive: Transition,
    going_in: bool,
}
impl Default for Interpolation {
    fn default() -> Self {
        Self {
            current: 0.,
            previous: 0.,
            previous_target: 0.,
            active: Transition::default(),
            inactive: Transition::default(),
            going_in: true,
        }
    }
}
impl Interpolation {
    fn timed(&mut self, target: f64, dt: f64, c: &Config) {
        self.previous = self.current;
        let mut swapped = false;
        if target != self.current {
            let increasing = target > self.current;
            self.active = if increasing {
                c.zoom_in_transition
            } else {
                c.zoom_out_transition.opposite()
            };
            self.inactive = if increasing {
                c.zoom_out_transition.opposite()
            } else {
                c.zoom_in_transition
            };
            if (increasing && self.previous_target < target
                || !increasing && self.previous_target > target)
                && self.active.inverse(0.).is_some()
            {
                self.current = self
                    .active
                    .inverse(self.inactive.apply(self.current))
                    .unwrap();
                swapped = true;
            }
        }
        self.previous_target = target;
        if self.active == Transition::Instant {
            self.current = target;
        } else if target > self.current {
            self.going_in = true;
            self.current = (self.current + dt / c.zoom_in_time).min(target);
        } else if target < self.current {
            self.going_in = false;
            self.current = (self.current - dt / c.zoom_out_time).max(target);
        } else {
            self.going_in = true;
        }
        if swapped {
            self.previous = self
                .active
                .inverse(self.inactive.apply(self.previous))
                .unwrap();
        }
        if !self.smooth(c) {
            self.previous = self.current;
        }
    }
    fn smooth(&self, c: &Config) -> bool {
        self.active != Transition::Instant
            && if self.going_in {
                c.zoom_in_time > 0.
            } else {
                c.zoom_out_time > 0.
            }
    }
    fn sample(&self, alpha: f64, c: &Config) -> f64 {
        if self.smooth(c) {
            self.active
                .apply(self.previous + (self.current - self.previous) * alpha)
        } else {
            self.current
        }
    }
}
#[derive(Clone, Debug)]
pub struct Helper {
    initial: Interpolation,
    scroll: f64,
    previous_scroll: f64,
    last_tier: i32,
    resetting: bool,
    reset_multiplier: f64,
    zooming_last: bool,
}
impl Default for Helper {
    fn default() -> Self {
        Self {
            initial: Default::default(),
            scroll: 0.,
            previous_scroll: 0.,
            last_tier: 0,
            resetting: false,
            reset_multiplier: 0.,
            zooming_last: false,
        }
    }
}
impl Helper {
    pub fn reset(&mut self) {
        if !self.resetting && self.scroll != 0. {
            self.resetting = true;
            self.scroll = 0.;
            self.previous_scroll = 0.;
        }
    }
    pub fn tick(&mut self, zooming: bool, tier: i32, dt: f64, c: &Config) {
        if zooming && !self.zooming_last {
            self.resetting = false;
        }
        self.initial.timed(if zooming { 1. } else { 0. }, dt, c);
        self.zooming_last = zooming;
        if tier.abs() > self.last_tier.abs() {
            self.resetting = false;
        }
        let target = if c.scroll_step_count > 0 {
            tier as f64 / c.scroll_step_count as f64
        } else {
            0.
        };
        self.previous_scroll = self.scroll;
        if c.scroll_zoom_smoothness == 100 {
            self.scroll = target;
        } else {
            let amount =
                (target - self.scroll) * (c.scroll_zoom_smoothness as f64 / 100.) / 0.05 * dt;
            self.scroll = if target > self.scroll {
                (self.scroll + amount).min(target)
            } else {
                (self.scroll + amount).max(target)
            };
        }
        if !self.initial.smooth(c) {
            self.initial.previous = self.initial.current;
        }
        self.last_tier = tier;
    }
    pub fn divisor(&mut self, alpha: f64, c: &Config) -> f64 {
        let progress = self.initial.sample(alpha, c);
        let multiplier = 1.
            + (if self.resetting {
                self.reset_multiplier
            } else {
                1. / c.initial_zoom as f64
            } - 1.)
                * progress;
        let scroll = if self.resetting {
            0.
        } else if c.scroll_zoom_smoothness == 100 {
            self.scroll
        } else {
            self.previous_scroll + (self.scroll - self.previous_scroll) * alpha
        };
        let divisor = ((1. / multiplier)
            * (c.zoom_per_step as f64 / 100.).powf(scroll * c.scroll_step_count as f64))
        .clamp(1., 500.);
        if self.initial.current == 0. && self.scroll == 0. {
            self.resetting = false;
        }
        if !self.resetting {
            self.reset_multiplier = 1. / divisor;
        }
        divisor
    }
}
#[derive(Default)]
pub struct Controller {
    pub helper: Helper,
    held: bool,
    toggled: bool,
    pub tier: i32,
    accumulator: f64,
    pub divisor: f64,
}
impl Controller {
    pub fn key(&mut self, down: bool, c: &Config) {
        if down && !self.held && c.zoom_key_behaviour == KeyBehavior::Toggle {
            self.toggled = !self.toggled;
        }
        self.held = down;
    }
    pub fn active(&self, c: &Config) -> bool {
        c.enabled
            && match c.zoom_key_behaviour {
                KeyBehavior::Hold => self.held,
                KeyBehavior::Toggle => self.toggled,
            }
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
        self.divisor = 1.;
    }
    pub fn scroll(&mut self, steps: i32, c: &Config) -> bool {
        if !self.active(c) || !c.scroll_zoom {
            return false;
        }
        self.tier = self
            .tier
            .saturating_add(steps)
            .clamp(c.min_tier(), c.scroll_step_count);
        true
    }
    pub fn frame(&mut self, dt: f64, c: &Config) -> f64 {
        let active = self.active(c);
        self.accumulator += dt.min(1.);
        while self.accumulator >= 0.05 {
            if !active && !c.retain_zoom_steps {
                self.helper.reset();
                self.tier = 0;
            }
            self.helper.tick(active, self.tier, 0.05, c);
            self.accumulator -= 0.05;
        }
        self.divisor = self.helper.divisor(self.accumulator / 0.05, c);
        self.divisor
    }
    pub fn sensitivity(&self, c: &Config) -> f64 {
        1. / (1. + (self.divisor.max(1.) - 1.) * c.relative_sensitivity as f64 / 100.)
    }
    pub fn bob_scale(&self, c: &Config) -> f32 {
        if c.relative_view_bobbing {
            (1. / (1. + (self.divisor.max(1.) - 1.) * 0.2)) as f32
        } else {
            1.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    fn reference() -> serde_json::Value {
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/mods/zoomify-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut data)
        .unwrap();
        serde_json::from_slice(&data).unwrap()
    }
    #[test]
    fn all_fourteen_curves_and_available_inverses_match_downloaded_mod() {
        for curve in reference()["curves"].as_array().unwrap() {
            let transition: Transition = serde_json::from_value(curve["name"].clone()).unwrap();
            for sample in curve["samples"].as_array().unwrap() {
                let x = sample[0].as_f64().unwrap();
                let expected = sample[1].as_f64().unwrap();
                assert!(
                    (transition.apply(x) - expected).abs() < 1e-12,
                    "{} {x}: {} != {expected}",
                    curve["name"],
                    transition.apply(x)
                );
                if let Some(expected) = sample[2].as_f64() {
                    assert!(
                        (transition.inverse(x).unwrap() - expected).abs() < 1e-12,
                        "inverse {} {x}",
                        curve["name"]
                    );
                }
            }
        }
    }
    #[test]
    fn original_zoom_helper_hold_scroll_negative_tiers_release_and_reversal_trace() {
        let reference = reference();
        let config = Config::default();
        assert_eq!(
            config.min_tier(),
            reference["min_tier"].as_i64().unwrap() as i32
        );
        let mut helper = Helper::default();
        for (index, row) in reference["trace"].as_array().unwrap().iter().enumerate() {
            if row["reset"].as_bool().unwrap() {
                helper.reset();
            }
            helper.tick(
                row["hold"].as_bool().unwrap(),
                row["tier"].as_i64().unwrap() as i32,
                0.05,
                &config,
            );
            for (alpha, expected) in [0., 0.25, 0.5, 1.]
                .into_iter()
                .zip(row["divisors"].as_array().unwrap())
            {
                let actual = helper.divisor(alpha, &config);
                assert!(
                    (actual - expected.as_f64().unwrap()).abs() < 1e-9,
                    "tick {index}, alpha {alpha}: {actual} != {expected}"
                );
            }
        }
    }
}
