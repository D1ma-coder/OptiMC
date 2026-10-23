//! Bounded local packet metadata and decoded server disconnect text, no raw bodies.
use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Clone)]
pub struct Journal {
    inner: Arc<Mutex<Log>>,
}
#[derive(Clone, Serialize)]
struct Event {
    ms: u64,
    phase: &'static str,
    direction: &'static str,
    id: i32,
    packet: &'static str,
    body_bytes: usize,
}
#[derive(Serialize)]
struct Summary {
    protocol: i32,
    started_unix_ms: u128,
    elapsed_ms: u64,
    phase: &'static str,
    sent: u64,
    received: u64,
    maximum_outgoing_in_one_second: usize,
    counts: BTreeMap<String, u64>,
    recent: VecDeque<Event>,
    outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_disconnect_reason: Option<String>,
    compression_threshold: Option<usize>,
    decoded_chunks: VecDeque<ChunkSummary>,
}
#[derive(Serialize)]
struct ChunkSummary {
    x: i32,
    z: i32,
    sections: usize,
    declared_non_air: usize,
    decoded_nonzero_states: usize,
}
struct Log {
    start: Instant,
    last_flush: Instant,
    path: PathBuf,
    summary: Summary,
    window: VecDeque<(u64, usize)>,
    window_count: usize,
}
impl Journal {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = dir.join(format!("session-{stamp}-{}.json", std::process::id()));
        let now = Instant::now();
        let log = Log {
            start: now,
            last_flush: now,
            path,
            window: VecDeque::new(),
            window_count: 0,
            summary: Summary {
                protocol: crate::assets::PROTOCOL,
                started_unix_ms: stamp / 1_000_000,
                elapsed_ms: 0,
                phase: "handshake",
                sent: 0,
                received: 0,
                maximum_outgoing_in_one_second: 0,
                counts: BTreeMap::new(),
                recent: VecDeque::new(),
                outcome: "running".into(),
                server_disconnect_reason: None,
                compression_threshold: None,
                decoded_chunks: VecDeque::new(),
            },
        };
        let journal = Self {
            inner: Arc::new(Mutex::new(log)),
        };
        journal.flush()?;
        // Only this journal's generated files are retained; unrelated files are untouched.
        let mut paths = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("session-") && n.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        let remove = paths.len().saturating_sub(16);
        for path in paths.into_iter().take(remove) {
            let _ = std::fs::remove_file(path);
        }
        Ok(journal)
    }
    pub fn phase(&self, phase: &'static str) {
        self.inner.lock().unwrap().summary.phase = phase;
    }
    pub fn record(&self, direction: &'static str, id: i32, bytes: usize) {
        let mut log = self.inner.lock().unwrap();
        let ms = log.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let phase = log.summary.phase;
        let flow = if direction == "send" {
            "serverbound"
        } else {
            "clientbound"
        };
        let packet = super::protocol::name(phase, flow, id).unwrap_or("unknown_packet_id");
        let key = format!("{phase}/{direction}/{packet}");
        *log.summary.counts.entry(key).or_default() += 1;
        if direction == "send" {
            log.summary.sent += 1;
            while log
                .window
                .front()
                .is_some_and(|(old, _)| ms.saturating_sub(*old) >= 1000)
            {
                let (_, count) = log.window.pop_front().unwrap();
                log.window_count -= count;
            }
            if let Some((last, count)) = log.window.back_mut().filter(|(last, _)| *last == ms) {
                let _ = last;
                *count += 1;
            } else {
                log.window.push_back((ms, 1));
            }
            log.window_count += 1;
            log.summary.maximum_outgoing_in_one_second = log
                .summary
                .maximum_outgoing_in_one_second
                .max(log.window_count);
        } else {
            log.summary.received += 1;
        }
        if log.summary.recent.len() == 256 {
            log.summary.recent.pop_front();
        }
        log.summary.recent.push_back(Event {
            ms,
            phase,
            direction,
            id,
            packet,
            body_bytes: bytes,
        });
    }
    pub fn finish(&self, reason: &str) {
        self.inner.lock().unwrap().summary.outcome = reason.chars().take(2048).collect();
        let _ = self.flush();
    }
    pub fn compression(&self, threshold: Option<usize>) {
        self.inner.lock().unwrap().summary.compression_threshold = threshold;
    }
    pub fn chunk(&self, chunk: &super::chunk::Chunk) {
        let summary = ChunkSummary {
            x: chunk.x,
            z: chunk.z,
            sections: chunk.sections.len(),
            declared_non_air: chunk.sections.iter().map(|s| s.non_air as usize).sum(),
            decoded_nonzero_states: chunk
                .sections
                .iter()
                .map(|s| s.blocks.iter().filter(|&&state| state != 0).count())
                .sum(),
        };
        let mut log = self.inner.lock().unwrap();
        if log.summary.decoded_chunks.len() == 16 {
            log.summary.decoded_chunks.pop_front();
        }
        log.summary.decoded_chunks.push_back(summary);
    }
    pub fn server_disconnect(&self, text: &crate::text::RichText) {
        self.inner.lock().unwrap().summary.server_disconnect_reason =
            Some(text.plain_text().chars().take(4096).collect());
    }
    pub fn flush_if_due(&self) {
        let due = self.inner.lock().unwrap().last_flush.elapsed().as_secs() >= 2;
        if due {
            let _ = self.flush();
        }
    }
    fn flush(&self) -> Result<()> {
        let mut log = self.inner.lock().unwrap();
        log.summary.elapsed_ms = log.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let bytes = serde_json::to_vec_pretty(&log.summary)?;
        let tmp = log.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, &log.path)?;
        log.last_flush = Instant::now();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoded_disconnect_text_is_bounded_and_compression_state_is_recorded() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "optimc-journal-reason-{}-{stamp}",
            std::process::id()
        ));
        let journal = Journal::open(&dir).unwrap();
        journal.compression(Some(256));
        assert_eq!(
            journal.inner.lock().unwrap().summary.compression_threshold,
            Some(256)
        );
        journal.compression(None);
        journal.server_disconnect(&crate::text::RichText::plain(
            &"§cПричина отказа. ".repeat(500),
        ));
        journal.finish("server_disconnect");
        let path = journal.inner.lock().unwrap().path.clone();
        let log: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let reason = log["server_disconnect_reason"].as_str().unwrap();
        assert_eq!(reason.chars().count(), 4096);
        assert!(reason.starts_with("Причина отказа."));
        assert!(!reason.contains('§'));
        assert!(log["compression_threshold"].is_null());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn metadata_is_bounded_phase_aware_and_never_contains_payloads() {
        let dir = std::env::temp_dir().join(format!(
            "optimc-journal-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let j = Journal::open(&dir).unwrap();
        j.phase("play");
        for _ in 0..5000 {
            j.record("send", 0x1e, 25);
            j.record("receive", i32::MAX, 1000);
        }
        for x in 0..20 {
            let mut blocks = vec![0; 4096];
            blocks[0] = 1;
            j.chunk(&super::super::chunk::Chunk {
                block_entities: vec![],
                x,
                z: -1,
                sections: vec![super::super::chunk::Section {
                    non_air: 1,
                    fluids: 0,
                    blocks,
                    biomes: vec![0; 64],
                }],
            });
        }
        j.finish("closed");
        let path = std::fs::read_dir(&dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(v["sent"], 5000);
        assert_eq!(v["received"], 5000);
        assert_eq!(v["recent"].as_array().unwrap().len(), 256);
        assert_eq!(v["counts"].as_object().unwrap().len(), 2);
        assert_eq!(v["outcome"], "closed");
        assert_eq!(v["decoded_chunks"].as_array().unwrap().len(), 16);
        assert_eq!(v["decoded_chunks"][0]["x"], 4);
        assert_eq!(v["decoded_chunks"][15]["declared_non_air"], 1);
        assert_eq!(v["decoded_chunks"][15]["decoded_nonzero_states"], 1);
        assert!(v["decoded_chunks"][15].get("blocks").is_none());
        assert!(j.inner.lock().unwrap().window.len() <= 1000);
        assert!(v["maximum_outgoing_in_one_second"].as_u64().unwrap() <= 5000);
        assert_eq!(v["recent"][254]["packet"], "move_player_pos");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
