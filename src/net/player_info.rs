//! Protocol 776 player-list state. Chat keys are retained as unverified data;
//! signed-chat authentication and message-chain validation are separate work.
use super::{
    codec::{read_string, read_varint},
    component,
    maps::{byte, bytes},
    scoreboard,
};
use crate::text::RichText;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, sync::Arc};
pub type Id = [u8; 16];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub properties: Vec<Property>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSession {
    pub id: Id,
    pub expires_at_ms: i64,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: Id,
    pub profile: Profile,
    pub listed: bool,
    pub latency: i32,
    pub game_mode: u8,
    pub display: Option<RichText>,
    pub show_hat: bool,
    pub order: i32,
    pub chat: Option<ChatSession>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    pub players: BTreeMap<Id, Arc<Player>>,
    pub header: RichText,
    pub footer: RichText,
    pub revision: u64,
}
struct Update {
    id: Id,
    profile: Option<Profile>,
    chat: Option<Option<ChatSession>>,
    mode: Option<u8>,
    listed: Option<bool>,
    latency: Option<i32>,
    display: Option<Option<RichText>>,
    order: Option<i32>,
    hat: Option<bool>,
}
fn boolean(input: &mut &[u8]) -> Result<bool> {
    Ok(byte(input)? != 0)
}
fn id(input: &mut &[u8]) -> Result<Id> {
    Ok(bytes(input, 16)?.try_into()?)
}
fn count(input: &mut &[u8], max: usize) -> Result<usize> {
    let n = read_varint(input)?;
    ensure!(
        n >= 0 && n as usize <= max && n as usize <= input.len(),
        "Invalid player-info collection size"
    );
    Ok(n as usize)
}
fn blob(input: &mut &[u8], max: usize) -> Result<Vec<u8>> {
    let n = count(input, max)?;
    Ok(bytes(input, n)?.to_vec())
}
impl Store {
    pub fn packet(&mut self, packet: i32, mut input: &[u8]) -> Result<()> {
        match packet {
            0x46 => {
                let actions = byte(&mut input)?;
                let n = count(&mut input, 16384)?;
                let mut updates = Vec::with_capacity(n);
                for _ in 0..n {
                    let mut u = Update {
                        id: id(&mut input)?,
                        profile: None,
                        chat: None,
                        mode: None,
                        listed: None,
                        latency: None,
                        display: None,
                        order: None,
                        hat: None,
                    };
                    if actions & 1 != 0 {
                        let name = read_string(&mut input, 16)?;
                        let n = count(&mut input, 1024)?;
                        let mut properties = Vec::with_capacity(n);
                        for _ in 0..n {
                            properties.push(Property {
                                name: read_string(&mut input, 32767)?,
                                value: read_string(&mut input, 32767)?,
                                signature: if boolean(&mut input)? {
                                    Some(read_string(&mut input, 32767)?)
                                } else {
                                    None
                                },
                            });
                        }
                        u.profile = Some(Profile { name, properties });
                    }
                    if actions & 2 != 0 {
                        u.chat = Some(if boolean(&mut input)? {
                            Some(ChatSession {
                                id: id(&mut input)?,
                                expires_at_ms: i64::from_be_bytes(
                                    bytes(&mut input, 8)?.try_into()?,
                                ),
                                public_key: blob(&mut input, 512)?,
                                signature: blob(&mut input, 4096)?,
                            })
                        } else {
                            None
                        });
                    }
                    if actions & 4 != 0 {
                        let mode = read_varint(&mut input)?;
                        u.mode = Some(if (0..=3).contains(&mode) {
                            mode as u8
                        } else {
                            0
                        });
                    }
                    if actions & 8 != 0 {
                        u.listed = Some(boolean(&mut input)?);
                    }
                    if actions & 16 != 0 {
                        u.latency = Some(read_varint(&mut input)?);
                    }
                    if actions & 32 != 0 {
                        u.display = Some(if boolean(&mut input)? {
                            Some(component::read_rich(&mut input)?)
                        } else {
                            None
                        });
                    }
                    if actions & 64 != 0 {
                        u.order = Some(read_varint(&mut input)?);
                    }
                    if actions & 128 != 0 {
                        u.hat = Some(boolean(&mut input)?);
                    }
                    updates.push(u);
                }
                ensure!(input.is_empty(), "Trailing player-info update bytes");
                // Original listener creates all new profiles before applying any actions.
                let incoming = updates
                    .iter()
                    .filter(|u| u.profile.is_some() && !self.players.contains_key(&u.id))
                    .map(|u| u.id)
                    .collect::<std::collections::BTreeSet<_>>();
                ensure!(
                    self.players.len() + incoming.len() <= 16384,
                    "Player-info state limit exceeded"
                );
                for u in &updates {
                    if let Some(profile) = &u.profile {
                        self.players.entry(u.id).or_insert_with(|| {
                            Arc::new(Player {
                                id: u.id,
                                profile: profile.clone(),
                                listed: false,
                                latency: 0,
                                game_mode: 0,
                                display: None,
                                show_hat: true,
                                order: 0,
                                chat: None,
                            })
                        });
                    }
                }
                for u in updates {
                    if let Some(p) = self.players.get_mut(&u.id) {
                        let p = Arc::make_mut(p);
                        if let Some(v) = u.chat {
                            p.chat = v;
                        }
                        if let Some(v) = u.mode {
                            p.game_mode = v;
                        }
                        if let Some(v) = u.listed {
                            p.listed = v;
                        }
                        if let Some(v) = u.latency {
                            p.latency = v;
                        }
                        if let Some(v) = u.display {
                            p.display = v;
                        }
                        if let Some(v) = u.order {
                            p.order = v;
                        }
                        if let Some(v) = u.hat {
                            p.show_hat = v;
                        }
                    }
                }
            }
            0x45 => {
                let n = count(&mut input, 16384)?;
                let mut ids = Vec::with_capacity(n);
                for _ in 0..n {
                    ids.push(id(&mut input)?);
                }
                ensure!(input.is_empty(), "Trailing player removal bytes");
                for id in ids {
                    self.players.remove(&id);
                }
            }
            0x7a => {
                let header = component::read_rich(&mut input)?;
                let footer = component::read_rich(&mut input)?;
                ensure!(input.is_empty(), "Trailing player-list header/footer bytes");
                self.header = header;
                self.footer = footer;
            }
            _ => anyhow::bail!("Not a player-info packet"),
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn listed<'a>(&'a self, scores: &scoreboard::Store) -> Vec<&'a Player> {
        let mut players: Vec<_> = self
            .players
            .values()
            .filter(|p| p.listed)
            .map(Arc::as_ref)
            .collect();
        players.sort_by(|a, b| {
            a.order
                .wrapping_neg()
                .cmp(&b.order.wrapping_neg())
                .then_with(|| (a.game_mode == 3).cmp(&(b.game_mode == 3)))
                .then_with(|| {
                    scores
                        .team_name(&a.profile.name)
                        .encode_utf16()
                        .cmp(scores.team_name(&b.profile.name).encode_utf16())
                })
                .then_with(|| super::java_case::compare(&a.profile.name, &b.profile.name))
        });
        players.truncate(80);
        players
    }
    pub fn display_name(player: &Player, scores: &scoreboard::Store) -> RichText {
        let mut name = player
            .display
            .clone()
            .unwrap_or_else(|| scores.formatted_name(&player.profile.name, None));
        if player.game_mode == 3 {
            for run in &mut name.runs {
                run.style.italic = true;
            }
        }
        name
    }
}
pub fn ping_sprite(latency: i32) -> &'static str {
    match latency {
        i32::MIN..=-1 => "tab_ping_unknown",
        0..=149 => "tab_ping_5",
        150..=299 => "tab_ping_4",
        300..=599 => "tab_ping_3",
        600..=999 => "tab_ping_2",
        _ => "tab_ping_1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    pub(crate) fn packets() -> Vec<(i32, Vec<u8>)> {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            &include_bytes!("../../data/26.2/player-info-reference.json.gz")[..],
        )
        .read_to_end(&mut bytes)
        .unwrap();
        use sha1::{Digest, Sha1};
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
        assert_eq!(
            format!("{:x}", Sha1::digest(&bytes)),
            reference["sha1"]["player-info-reference.json"]
                .as_str()
                .unwrap()
        );
        let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        data["packets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["id"].as_i64().unwrap() as i32,
                    base64::engine::general_purpose::STANDARD
                        .decode(p["body"].as_str().unwrap())
                        .unwrap(),
                )
            })
            .collect()
    }
    fn uuid(n: u8) -> Id {
        let mut id = [0; 16];
        id[7] = 1;
        id[15] = n;
        id
    }
    #[test]
    fn all_eight_original_actions_preserve_profile_and_update_their_fields() {
        let packets = packets();
        let mut store = Store::default();
        store.packet(packets[0].0, &packets[0].1).unwrap();
        let p = &store.players[&uuid(1)];
        assert_eq!(p.profile.name, "Alice");
        assert_eq!(p.profile.properties.len(), 2);
        assert!(p
            .profile
            .properties
            .iter()
            .any(|p| p.signature.as_deref() == Some("signature")));
        assert_eq!(p.chat.as_ref().unwrap().expires_at_ms, 2000000000000);
        assert!(p.chat.as_ref().unwrap().public_key.len() > 200);
        assert_eq!(p.display.as_ref().unwrap().plain_text(), "Alice display");
        assert_eq!(
            p.display.as_ref().unwrap().runs[0].style.color,
            Some(0xffaa00)
        );
        assert!(!p.show_hat);
        assert_eq!(p.order, 7);
        assert_eq!(p.latency, 42);
        assert_eq!(store.listed(&scoreboard::Store::default()).len(), 2);
        for (index, (id, body)) in packets.iter().enumerate().take(9).skip(1) {
            let mut updated = store.clone();
            updated.packet(*id, body).unwrap();
            let changed = &updated.players[&uuid(1)];
            assert_eq!(changed.profile.name, "Alice");
            match index {
                1 => assert_eq!(changed.profile, p.profile),
                2 => assert!(changed.chat.is_none()),
                3 => assert_eq!(changed.game_mode, 2),
                4 => assert!(!changed.listed),
                5 => assert_eq!(changed.latency, 800),
                6 => assert!(changed.display.is_none()),
                7 => assert_eq!(changed.order, 12),
                8 => assert!(changed.show_hat),
                _ => unreachable!(),
            }
        }
        store.packet(packets[9].0, &packets[9].1).unwrap();
        assert_eq!(store.header.plain_text(), "Header\nSecond line");
        store.packet(packets[10].0, &packets[10].1).unwrap();
        assert!(store.header.runs.is_empty() && store.footer.runs.is_empty());
        store.packet(packets[11].0, &packets[11].1).unwrap();
        assert_eq!(store.players.len(), 1);
        assert!(store.players.contains_key(&uuid(2)));
    }
    #[test]
    fn truncated_updates_removals_and_header_footer_are_transactional() {
        let packets = packets();
        let mut initial = Store::default();
        initial.packet(packets[0].0, &packets[0].1).unwrap();
        for (id, body) in packets {
            for end in 0..body.len() {
                let mut store = initial.clone();
                assert!(store.packet(id, &body[..end]).is_err(), "{id:x} {end}");
                assert_eq!(store, initial);
            }
        }
    }
    #[test]
    fn unknown_updates_do_not_create_phantom_players_and_ping_thresholds_match_original() {
        let packets = packets();
        let mut store = Store::default();
        store.packet(packets[5].0, &packets[5].1).unwrap();
        assert!(store.players.is_empty());
        for (latency, expected) in [
            (-1, "tab_ping_unknown"),
            (0, "tab_ping_5"),
            (149, "tab_ping_5"),
            (150, "tab_ping_4"),
            (299, "tab_ping_4"),
            (300, "tab_ping_3"),
            (599, "tab_ping_3"),
            (600, "tab_ping_2"),
            (999, "tab_ping_2"),
            (1000, "tab_ping_1"),
        ] {
            assert_eq!(ping_sprite(latency), expected);
        }
    }
}
