//! Client-side LevelLoadTracker for external worlds (26.2). A compiled section
//! alone is insufficient: LEVEL_CHUNKS_LOAD_START must arrive first.
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Inactive,
    WaitingForServer,
    WaitingForPlayerChunk,
    Ready,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Tracker {
    stage: Stage,
    started: Option<Instant>,
    compiled: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    pub player_outside_height: bool,
    pub camera_outside_height: bool,
    pub spectator: bool,
    pub alive: bool,
}
impl Tracker {
    pub fn has_loading_packets(&self) -> bool {
        matches!(self.stage, Stage::WaitingForPlayerChunk | Stage::Ready)
    }
    pub fn start(&mut self, now: Instant) {
        *self = Self {
            stage: Stage::WaitingForServer,
            started: Some(now),
            compiled: false,
        };
    }
    pub fn loading_packets_received(&mut self) {
        if self.stage == Stage::WaitingForServer {
            self.stage = Stage::WaitingForPlayerChunk;
        }
    }
    pub fn compiled_section(&mut self) {
        // Original callback is null before loadingPacketsReceived, and captures
        // only that WaitingForPlayerChunk instance; it cannot complete a reset.
        if self.stage == Stage::WaitingForPlayerChunk {
            self.compiled = true;
        }
    }
    pub fn tick(&mut self, now: Instant, conditions: Conditions) -> bool {
        if self.stage == Stage::WaitingForPlayerChunk
            && (self
                .started
                .is_some_and(|start| now.saturating_duration_since(start).as_millis() > 30_000)
                || conditions.player_outside_height
                || conditions.camera_outside_height
                || conditions.spectator
                || !conditions.alive
                || self.compiled)
        {
            self.stage = Stage::Ready;
        }
        self.stage == Stage::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn ordinary() -> Conditions {
        Conditions {
            player_outside_height: false,
            camera_outside_height: false,
            spectator: false,
            alive: true,
        }
    }
    #[test]
    fn original_codec_and_tracker_transitions_are_preserved() {
        use base64::Engine;
        use sha1::{Digest, Sha1};
        let raw = include_str!("../../data/26.2/level-loading.json");
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
        assert_eq!(
            format!("{:x}", Sha1::digest(raw.as_bytes())),
            manifest["sha1"]["level-loading.json"]
        );
        let r: serde_json::Value = serde_json::from_str(raw).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(r["body"].as_str().unwrap())
            .unwrap();
        assert_eq!(r["id"], 0x26);
        assert_eq!(bytes, [13, 0, 0, 0, 0]);
        assert_eq!(r["callbackBeforeMarker"], false);
        assert_eq!(r["stageAfterMarker"], "WaitingForPlayerChunk");
        for key in [
            "callbackAfterMarker",
            "duplicateKeepsState",
            "compiled",
            "resetDiscardsOldCallback",
            "readyAfterTimeout",
        ] {
            assert_eq!(r[key], true);
        }
        let at = Instant::now();
        let mut tracker = Tracker::default();
        tracker.start(at);
        assert!(!tracker.has_loading_packets());
        tracker.loading_packets_received();
        assert!(tracker.has_loading_packets());
        tracker.compiled_section();
        tracker.loading_packets_received();
        assert!(tracker.tick(at, ordinary()));
        tracker.start(at);
        assert!(!tracker.has_loading_packets());
    }
    #[test]
    fn marker_gates_every_completion_path_and_early_callback_is_ignored() {
        let at = Instant::now();
        let mut tracker = Tracker::default();
        tracker.start(at);
        tracker.compiled_section();
        assert!(!tracker.tick(
            at + Duration::from_secs(31),
            Conditions {
                spectator: true,
                ..ordinary()
            }
        ));
        tracker.loading_packets_received();
        assert!(!tracker.tick(at, ordinary()));
        tracker.compiled_section();
        tracker.loading_packets_received();
        assert!(tracker.tick(at, ordinary()));
        tracker.start(at);
        assert!(!tracker.tick(at, ordinary()));
        tracker.loading_packets_received();
        assert!(!tracker.tick(at, ordinary()));
    }
    #[test]
    fn original_timeout_boundary_and_exception_conditions() {
        let at = Instant::now();
        let mut tracker = Tracker::default();
        tracker.start(at);
        tracker.loading_packets_received();
        assert!(!tracker.tick(at + Duration::from_secs(30), ordinary()));
        assert!(!tracker.tick(
            at + Duration::from_secs(30) + Duration::from_nanos(1),
            ordinary()
        ));
        assert!(tracker.tick(
            at + Duration::from_secs(30) + Duration::from_millis(1),
            ordinary()
        ));
        for conditions in [
            Conditions {
                player_outside_height: true,
                ..ordinary()
            },
            Conditions {
                camera_outside_height: true,
                ..ordinary()
            },
            Conditions {
                spectator: true,
                ..ordinary()
            },
            Conditions {
                alive: false,
                ..ordinary()
            },
        ] {
            tracker.start(at);
            tracker.loading_packets_received();
            assert!(tracker.tick(at, conditions));
        }
    }
}
