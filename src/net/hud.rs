//! Server-owned HUD state. No local progress/experience is invented.
use super::{codec::read_varint, component};
use crate::text::RichText;
use anyhow::{bail, ensure, Result};
use std::{io::Read, time::Instant};
fn float(data: &mut &[u8]) -> Result<f32> {
    let mut b = [0; 4];
    data.read_exact(&mut b)?;
    let f = f32::from_be_bytes(b);
    ensure!(f.is_finite(), "Non-finite HUD value");
    Ok(f)
}
fn byte(data: &mut &[u8]) -> Result<u8> {
    let mut b = [0];
    data.read_exact(&mut b)?;
    Ok(b[0])
}
#[derive(Clone, Debug)]
pub struct Boss {
    pub id: [u8; 16],
    pub name: RichText,
    pub target: f32,
    previous: f32,
    changed: Instant,
    pub color: u8,
    pub overlay: u8,
    pub flags: u8,
}
impl Boss {
    pub fn progress(&self) -> f32 {
        self.at(Instant::now())
    }
    fn at(&self, now: Instant) -> f32 {
        let t = now
            .saturating_duration_since(self.changed)
            .as_secs_f32()
            .mul_add(10., 0.)
            .clamp(0., 1.);
        self.previous + (self.target - self.previous) * t
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Experience {
    pub progress: f32,
    pub level: i32,
    pub total: i32,
}
#[derive(Clone, Debug)]
pub struct Titles {
    pub title: RichText,
    pub subtitle: RichText,
    pub fade_in: i32,
    pub stay: i32,
    pub fade_out: i32,
    started: Option<Instant>,
}
impl Default for Titles {
    fn default() -> Self {
        Self {
            title: RichText::default(),
            subtitle: RichText::default(),
            fade_in: 10,
            stay: 70,
            fade_out: 20,
            started: None,
        }
    }
}
impl Titles {
    pub fn alpha(&self) -> f32 {
        self.alpha_at(Instant::now())
    }
    fn alpha_at(&self, now: Instant) -> f32 {
        let Some(start) = self.started else {
            return 0.;
        };
        let ticks = now.saturating_duration_since(start).as_secs_f64() * 20.;
        let total = self.fade_in as f64 + self.stay as f64 + self.fade_out as f64;
        if ticks >= total {
            return 0.;
        }
        if ticks < self.fade_in as f64 {
            return (ticks / self.fade_in.max(1) as f64) as f32;
        }
        if ticks >= self.fade_in as f64 + self.stay as f64 {
            return ((total - ticks) / self.fade_out.max(1) as f64) as f32;
        }
        1.
    }
}
#[derive(Clone, Default)]
pub struct State {
    pub bosses: Vec<Boss>,
    pub experience: Experience,
    pub titles: Titles,
    pub action: Option<(RichText, Instant)>,
}
impl State {
    pub fn handles(id: i32) -> bool {
        matches!(id, 0x09 | 0x0e | 0x57 | 0x67 | 0x70 | 0x72 | 0x73)
    }
    /// Decode completely before replacing state, including validating trailing bytes.
    pub fn packet(&mut self, id: i32, mut data: &[u8]) -> Result<()> {
        let now = Instant::now();
        match id {
            0x09 => {
                let mut uuid = [0; 16];
                data.read_exact(&mut uuid)?;
                let op = read_varint(&mut data)?;
                enum Update {
                    Add(RichText, f32, u8, u8, u8),
                    Remove,
                    Progress(f32),
                    Name(RichText),
                    Style(u8, u8),
                    Flags(u8),
                }
                let style = |data: &mut &[u8]| -> Result<(u8, u8)> {
                    let color = read_varint(data)?;
                    let overlay = read_varint(data)?;
                    ensure!(
                        (0..7).contains(&color) && (0..5).contains(&overlay),
                        "Invalid boss style"
                    );
                    Ok((color as u8, overlay as u8))
                };
                let update = match op {
                    0 => {
                        let name = component::read_rich(&mut data)?;
                        let progress = float(&mut data)?;
                        let (color, overlay) = style(&mut data)?;
                        Update::Add(name, progress, color, overlay, byte(&mut data)?)
                    }
                    1 => Update::Remove,
                    2 => Update::Progress(float(&mut data)?),
                    3 => Update::Name(component::read_rich(&mut data)?),
                    4 => {
                        let (color, overlay) = style(&mut data)?;
                        Update::Style(color, overlay)
                    }
                    5 => Update::Flags(byte(&mut data)?),
                    _ => bail!("Unknown boss operation {op}"),
                };
                ensure!(data.is_empty(), "Trailing boss event");
                let index = self.bosses.iter().position(|b| b.id == uuid);
                match update {
                    Update::Add(name, progress, color, overlay, flags) => {
                        ensure!(
                            self.bosses.len() < 256 || index.is_some(),
                            "Too many bossbars"
                        );
                        let boss = Boss {
                            id: uuid,
                            name,
                            target: progress,
                            previous: progress,
                            changed: now,
                            color,
                            overlay,
                            flags,
                        };
                        if let Some(i) = index {
                            self.bosses[i] = boss;
                        } else {
                            self.bosses.push(boss);
                        }
                    }
                    Update::Remove => {
                        if let Some(i) = index {
                            self.bosses.remove(i);
                        }
                    }
                    other => {
                        if let Some(i) = index {
                            let boss = &mut self.bosses[i];
                            match other {
                                Update::Progress(p) => {
                                    boss.previous = boss.at(now);
                                    boss.target = p;
                                    boss.changed = now;
                                }
                                Update::Name(n) => boss.name = n,
                                Update::Style(c, o) => {
                                    boss.color = c;
                                    boss.overlay = o;
                                }
                                Update::Flags(f) => boss.flags = f,
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            }
            0x67 => {
                let progress = float(&mut data)?;
                let level = read_varint(&mut data)?;
                let total = read_varint(&mut data)?;
                ensure!(data.is_empty(), "Trailing experience packet");
                self.experience = Experience {
                    progress,
                    level,
                    total,
                };
            }
            0x57 | 0x70 | 0x72 => {
                let text = component::read_rich(&mut data)?;
                ensure!(data.is_empty(), "Trailing HUD component");
                match id {
                    0x57 => self.action = Some((text, now)),
                    0x70 => self.titles.subtitle = text,
                    _ => {
                        self.titles.title = text;
                        self.titles.started = Some(now);
                    }
                }
            }
            0x0e => {
                let reset = byte(&mut data)?;
                ensure!(reset <= 1 && data.is_empty(), "Invalid clear title");
                self.titles.title = RichText::default();
                self.titles.subtitle = RichText::default();
                self.titles.started = None;
                if reset != 0 {
                    self.titles = Titles::default();
                }
            }
            0x73 => {
                let mut times = [0; 3];
                for t in &mut times {
                    let mut b = [0; 4];
                    data.read_exact(&mut b)?;
                    *t = i32::from_be_bytes(b);
                }
                ensure!(data.is_empty(), "Trailing title timing");
                let active = self.titles.alpha_at(now) > 0.;
                for (target, value) in [
                    &mut self.titles.fade_in,
                    &mut self.titles.stay,
                    &mut self.titles.fade_out,
                ]
                .into_iter()
                .zip(times)
                {
                    if value >= 0 {
                        *target = value;
                    }
                }
                if active {
                    self.titles.started = Some(now);
                }
            }
            _ => bail!("Unsupported HUD packet"),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{
        codec::varint,
        nbt::{self, Tag},
    };
    use std::time::Duration;
    fn component(value: &str) -> Vec<u8> {
        let mut b = Vec::new();
        nbt::write_network(&Tag::String(value.into()), &mut b).unwrap();
        b
    }
    #[test]
    fn boss_add_update_remove_and_atomic_validation() {
        let mut s = State::default();
        let mut add = vec![1; 16];
        varint(0, &mut add);
        add.extend(component("§cBoss"));
        add.extend(0.75f32.to_be_bytes());
        add.extend([2, 3, 7]);
        s.packet(9, &add).unwrap();
        assert_eq!(s.bosses[0].name.plain_text(), "Boss");
        assert_eq!(s.bosses[0].flags, 7);
        let mut p = vec![1; 16];
        p.push(2);
        p.extend(0.25f32.to_be_bytes());
        s.packet(9, &p).unwrap();
        let b = &s.bosses[0];
        assert!((b.at(b.changed + Duration::from_millis(50)) - 0.5).abs() < 0.001);
        p.push(9);
        assert!(s.packet(9, &p).is_err());
        assert_eq!(s.bosses[0].target, 0.25);
        let mut remove = vec![1; 16];
        remove.push(1);
        s.packet(9, &remove).unwrap();
        assert!(s.bosses.is_empty());
    }
    #[test]
    fn title_subtitle_fades_and_reset() {
        let mut s = State::default();
        s.packet(0x70, &component("subtitle")).unwrap();
        assert_eq!(s.titles.alpha(), 0.);
        s.packet(0x72, &component("title")).unwrap();
        let start = s.titles.started.unwrap();
        assert_eq!(s.titles.alpha_at(start + Duration::from_secs(1)), 1.);
        assert!((s.titles.alpha_at(start + Duration::from_millis(4750)) - 0.25).abs() < 1e-6);
        s.packet(
            0x73,
            &[
                (-1i32).to_be_bytes(),
                20i32.to_be_bytes(),
                (-1i32).to_be_bytes(),
            ]
            .concat(),
        )
        .unwrap();
        assert_eq!(s.titles.stay, 20);
        assert_eq!(s.titles.fade_in, 10);
        s.packet(0x0e, &[1]).unwrap();
        assert_eq!(s.titles.stay, 70);
        assert_eq!(s.titles.alpha(), 0.);
    }
    #[test]
    fn experience_is_authoritative_and_truncation_does_not_mutate() {
        let mut s = State::default();
        let mut b = 0.75f32.to_be_bytes().to_vec();
        varint(42, &mut b);
        varint(12345, &mut b);
        s.packet(0x67, &b).unwrap();
        assert_eq!(
            s.experience,
            Experience {
                progress: 0.75,
                level: 42,
                total: 12345
            }
        );
        for end in 0..b.len() {
            assert!(s.packet(0x67, &b[..end]).is_err());
            assert_eq!(s.experience.level, 42);
        }
    }
}
