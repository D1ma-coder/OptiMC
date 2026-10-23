//! Server scoreboard state and sidebar projection, using the 26.2 packet codecs.
use super::{
    codec::{read_string, read_varint},
    component,
};
use crate::text::{RichText, Run, Style};
use anyhow::{bail, ensure, Result};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ENTRIES: usize = 16384;
const MAX_OBJECTIVES: usize = 256;
const MAX_TEAMS: usize = 1024;
const COLORS: [u32; 16] = [
    0, 0xaa, 0xaa00, 0xaaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa, 0x555555, 0x5555ff, 0x55ff55,
    0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumberFormat {
    Blank,
    Styled(Style),
    Fixed(RichText),
}

impl NumberFormat {
    fn read(input: &mut &[u8]) -> Result<Option<Self>> {
        if !boolean(input)? {
            return Ok(None);
        }
        Ok(Some(match read_varint(input)? {
            0 => Self::Blank,
            1 => Self::Styled(component::read_style(input)?),
            2 => Self::Fixed(component::read_rich(input)?),
            _ => bail!("Unknown scoreboard number format"),
        }))
    }
    fn format(&self, value: i32) -> RichText {
        match self {
            Self::Blank => RichText::default(),
            Self::Fixed(text) => text.clone(),
            Self::Styled(style) => RichText {
                runs: vec![Run {
                    text: value.to_string(),
                    style: *style,
                }],
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objective {
    pub title: RichText,
    pub hearts: bool,
    pub number_format: Option<NumberFormat>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    pub value: i32,
    pub display: Option<RichText>,
    pub number_format: Option<NumberFormat>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub display: RichText,
    pub prefix: RichText,
    pub suffix: RichText,
    pub visibility: u8,
    pub collision: u8,
    pub color: Option<u8>,
    pub options: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub parameters: Parameters,
    pub members: BTreeSet<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    pub objectives: BTreeMap<String, Objective>,
    pub scores: BTreeMap<String, BTreeMap<String, Score>>,
    pub slots: [Option<String>; 19],
    pub teams: BTreeMap<String, Team>,
    membership: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidebar {
    pub title: RichText,
    pub rows: Vec<(RichText, RichText)>,
}

fn byte(input: &mut &[u8]) -> Result<u8> {
    let (&value, rest) = input
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("Truncated scoreboard byte"))?;
    *input = rest;
    Ok(value)
}
fn boolean(input: &mut &[u8]) -> Result<bool> {
    Ok(byte(input)? != 0)
}
fn string(input: &mut &[u8]) -> Result<String> {
    read_string(input, 32767)
}
fn optional_text(input: &mut &[u8]) -> Result<Option<RichText>> {
    if boolean(input)? {
        Ok(Some(component::read_rich(input)?))
    } else {
        Ok(None)
    }
}
fn bounded_enum(input: &mut &[u8], max: i32) -> Result<u8> {
    let value = read_varint(input)?;
    ensure!((0..max).contains(&value), "Invalid scoreboard enum");
    Ok(value as u8)
}
enum Update {
    Objective(String, u8, Option<Objective>),
    Score(String, String, Score),
    Reset(String, Option<String>),
    Slot(usize, String),
    Team(String, u8, Option<Parameters>, Vec<String>),
}
impl Update {
    fn decode(id: i32, mut data: &[u8]) -> Result<Self> {
        let input = &mut data;
        let update = match id {
            0x6a => {
                let name = string(input)?;
                let method = byte(input)?;
                ensure!(method <= 2, "Invalid objective operation");
                let objective = if method != 1 {
                    Some(Objective {
                        title: component::read_rich(input)?,
                        hearts: bounded_enum(input, 2)? == 1,
                        number_format: NumberFormat::read(input)?,
                    })
                } else {
                    None
                };
                Self::Objective(name, method, objective)
            }
            0x6e => {
                let owner = string(input)?;
                let objective = string(input)?;
                Self::Score(
                    owner,
                    objective,
                    Score {
                        value: read_varint(input)?,
                        display: optional_text(input)?,
                        number_format: NumberFormat::read(input)?,
                    },
                )
            }
            0x4f => {
                let owner = string(input)?;
                let objective = if boolean(input)? {
                    Some(string(input)?)
                } else {
                    None
                };
                Self::Reset(owner, objective)
            }
            0x62 => Self::Slot(bounded_enum(input, 19)? as usize, string(input)?),
            0x6d => {
                let name = string(input)?;
                let method = byte(input)?;
                ensure!(method <= 4, "Invalid team operation");
                let parameters = if method == 0 || method == 2 {
                    Some(Parameters {
                        display: component::read_rich(input)?,
                        prefix: component::read_rich(input)?,
                        suffix: component::read_rich(input)?,
                        visibility: bounded_enum(input, 4)?,
                        collision: bounded_enum(input, 4)?,
                        color: if boolean(input)? {
                            Some(bounded_enum(input, 16)?)
                        } else {
                            None
                        },
                        options: byte(input)?,
                    })
                } else {
                    None
                };
                let mut players = vec![];
                if method == 0 || method == 3 || method == 4 {
                    let count = read_varint(input)?;
                    ensure!(
                        (0..=MAX_ENTRIES as i32).contains(&count),
                        "Team member limit exceeded"
                    );
                    for _ in 0..count {
                        players.push(string(input)?);
                    }
                }
                Self::Team(name, method, parameters, players)
            }
            _ => bail!("Not a scoreboard packet"),
        };
        ensure!(input.is_empty(), "Trailing scoreboard bytes");
        Ok(update)
    }
}
impl Store {
    pub fn handles(id: i32) -> bool {
        matches!(id, 0x4f | 0x62 | 0x6a | 0x6d | 0x6e)
    }
    /// Decode the entire update before changing any shared state.
    pub fn packet(&mut self, id: i32, data: &[u8]) -> Result<()> {
        match Update::decode(id, data)? {
            Update::Objective(name, method, objective) => match method {
                0 => {
                    ensure!(
                        !self.objectives.contains_key(&name),
                        "Duplicate scoreboard objective"
                    );
                    ensure!(
                        self.objectives.len() < MAX_OBJECTIVES,
                        "Scoreboard objective limit exceeded"
                    );
                    self.objectives.insert(name, objective.unwrap());
                }
                1 => {
                    self.objectives.remove(&name);
                    self.scores.remove(&name);
                    for slot in &mut self.slots {
                        if slot.as_ref() == Some(&name) {
                            *slot = None;
                        }
                    }
                }
                _ => {
                    if let Some(old) = self.objectives.get_mut(&name) {
                        *old = objective.unwrap();
                    }
                }
            },
            Update::Score(owner, objective, score) => {
                if self.objectives.contains_key(&objective) {
                    let present = self
                        .scores
                        .get(&objective)
                        .is_some_and(|s| s.contains_key(&owner));
                    ensure!(
                        present
                            || self.scores.values().map(BTreeMap::len).sum::<usize>() < MAX_ENTRIES,
                        "Scoreboard score limit exceeded"
                    );
                    self.scores
                        .entry(objective)
                        .or_default()
                        .insert(owner, score);
                }
            }
            Update::Reset(owner, objective) => {
                if let Some(objective) = objective {
                    if let Some(scores) = self.scores.get_mut(&objective) {
                        scores.remove(&owner);
                    }
                } else {
                    for scores in self.scores.values_mut() {
                        scores.remove(&owner);
                    }
                }
            }
            Update::Slot(slot, objective) => {
                self.slots[slot] = self
                    .objectives
                    .contains_key(&objective)
                    .then_some(objective)
            }
            Update::Team(name, method, parameters, players) => {
                if method == 0 {
                    ensure!(
                        self.teams.contains_key(&name) || self.teams.len() < MAX_TEAMS,
                        "Scoreboard team limit exceeded"
                    );
                } else if !self.teams.contains_key(&name) {
                    return Ok(());
                }
                if method == 4 {
                    ensure!(
                        players
                            .iter()
                            .all(|p| self.membership.get(p) == Some(&name)),
                        "Player is not on the removed team"
                    );
                }
                if method == 0 || method == 3 {
                    let new: BTreeSet<_> = players
                        .iter()
                        .filter(|p| !self.membership.contains_key(*p))
                        .collect();
                    ensure!(
                        self.membership.len() + new.len() <= MAX_ENTRIES,
                        "Scoreboard member limit exceeded"
                    );
                }
                if let Some(parameters) = parameters {
                    if let Some(team) = self.teams.get_mut(&name) {
                        team.parameters = parameters;
                    } else {
                        self.teams.insert(
                            name.clone(),
                            Team {
                                parameters,
                                members: BTreeSet::new(),
                            },
                        );
                    }
                }
                if method == 1 {
                    if let Some(team) = self.teams.remove(&name) {
                        for player in team.members {
                            self.membership.remove(&player);
                        }
                    }
                } else if method == 0 || method == 3 {
                    for player in players {
                        if let Some(old) = self.membership.insert(player.clone(), name.clone()) {
                            if let Some(team) = self.teams.get_mut(&old) {
                                team.members.remove(&player);
                            }
                        }
                        self.teams.get_mut(&name).unwrap().members.insert(player);
                    }
                } else if method == 4 {
                    for player in players {
                        if self.membership.get(&player) == Some(&name) {
                            self.membership.remove(&player);
                            self.teams.get_mut(&name).unwrap().members.remove(&player);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn team(&self, owner: &str) -> Option<&Team> {
        self.teams.get(self.membership.get(owner)?)
    }
    pub fn team_name(&self, owner: &str) -> &str {
        self.membership.get(owner).map_or("", String::as_str)
    }
    /// Integer list objective, including per-score/ objective number formats.
    /// Heart objectives need the separate animated player-list health renderer.
    pub fn list_score(&self, owner: &str) -> Option<RichText> {
        let objective = self.slots[0].as_ref()?;
        let definition = self.objectives.get(objective)?;
        if definition.hearts {
            return None;
        }
        let Some(score) = self.scores.get(objective).and_then(|s| s.get(owner)) else {
            return Some(RichText::default());
        };
        let fallback = NumberFormat::Styled(Style {
            color: Some(0xffff55),
            ..Default::default()
        });
        Some(
            score
                .number_format
                .as_ref()
                .or(definition.number_format.as_ref())
                .unwrap_or(&fallback)
                .format(score.value),
        )
    }
    pub fn formatted_name(&self, owner: &str, display: Option<&RichText>) -> RichText {
        let text = display.cloned().unwrap_or_else(|| RichText::plain(owner));
        let Some(team) = self.team(owner) else {
            return text;
        };
        let mut name = team.parameters.prefix.clone();
        name.runs.extend(text.runs);
        name.runs.extend(team.parameters.suffix.runs.clone());
        if let Some(color) = team.parameters.color {
            for run in &mut name.runs {
                if run.style.color.is_none() {
                    run.style.color = Some(COLORS[color as usize]);
                }
            }
        }
        name
    }
    pub fn sidebar(&self, player: &str) -> Option<Sidebar> {
        let colored = self
            .team(player)
            .and_then(|t| t.parameters.color)
            .and_then(|c| self.slots[3 + c as usize].as_ref());
        let id = colored.or(self.slots[1].as_ref())?;
        let objective = self.objectives.get(id)?;
        let mut rows: Vec<_> = self
            .scores
            .get(id)
            .into_iter()
            .flat_map(|s| s.iter())
            .filter(|(owner, _)| !owner.starts_with('#'))
            .collect();
        let compare = |a: &(&String, &Score), b: &(&String, &Score)| {
            b.1.value
                .cmp(&a.1.value)
                .then_with(|| super::java_case::compare(a.0, b.0))
        };
        if rows.len() > 15 {
            rows.select_nth_unstable_by(15, compare);
            rows.truncate(15);
        }
        rows.sort_by(compare);
        let default = NumberFormat::Styled(Style {
            color: Some(0xff5555),
            ..Default::default()
        });
        Some(Sidebar {
            title: objective.title.clone(),
            rows: rows
                .into_iter()
                .map(|(owner, score)| {
                    (
                        self.formatted_name(owner, score.display.as_ref()),
                        score
                            .number_format
                            .as_ref()
                            .or(objective.number_format.as_ref())
                            .unwrap_or(&default)
                            .format(score.value),
                    )
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;

    fn fixtures() -> Vec<(String, i32, Vec<u8>)> {
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(
            &include_bytes!("../../data/26.2/scoreboard-reference.json.gz")[..],
        )
        .read_to_end(&mut json)
        .unwrap();
        let reference: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../data/26.2/reference.json")).unwrap();
        use sha1::{Digest, Sha1};
        assert_eq!(
            format!("{:x}", Sha1::digest(&json)),
            reference["sha1"]["scoreboard-reference.json"]
                .as_str()
                .unwrap()
        );
        let data: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(data["client_sha1"], reference["client_sha1"]);
        data["packets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["label"].as_str().unwrap().into(),
                    p["id"].as_i64().unwrap() as i32,
                    base64::engine::general_purpose::STANDARD
                        .decode(p["body"].as_str().unwrap())
                        .unwrap(),
                )
            })
            .collect()
    }
    fn initial() -> Store {
        let mut state = Store::default();
        for (_, id, bytes) in fixtures().into_iter().take(9) {
            state.packet(id, &bytes).unwrap();
        }
        state
    }
    #[test]
    fn unknown_objectives_do_not_create_phantoms_and_clear_display_slots() {
        let packets = fixtures();
        let mut state = Store::default();
        state.packet(packets[2].1, &packets[2].2).unwrap();
        assert!(state.scores.is_empty());
        let update = packets.iter().find(|p| p.0 == "objective_update").unwrap();
        state.packet(update.1, &update.2).unwrap();
        assert!(state.objectives.is_empty());
        state = initial();
        let mut clear = vec![1];
        super::super::codec::string("missing", &mut clear);
        state.packet(0x62, &clear).unwrap();
        assert!(state.slots[1].is_none());
        assert!(state.sidebar("unknown").is_none());
    }
    #[test]
    fn reset_one_objective_preserves_the_owners_other_scores() {
        let mut state = initial();
        state
            .scores
            .insert("other".into(), state.scores["main"].clone());
        let reset = fixtures().into_iter().find(|p| p.0 == "reset_one").unwrap();
        state.packet(reset.1, &reset.2).unwrap();
        assert!(!state.scores["main"].contains_key("beta"));
        assert!(state.scores["other"].contains_key("beta"));
        let reset = fixtures().into_iter().find(|p| p.0 == "reset_all").unwrap();
        state.packet(reset.1, &reset.2).unwrap();
        assert!(state
            .scores
            .values()
            .all(|scores| !scores.contains_key("Alpha")));
    }
    #[test]
    fn original_codec_packets_preserve_styles_formats_and_all_team_operations() {
        let mut state = Store::default();
        for (label, id, bytes) in fixtures() {
            state
                .packet(id, &bytes)
                .unwrap_or_else(|e| panic!("{label}: {e:#}"));
            match label.as_str() {
                "team_add" => {
                    let side = state.sidebar("Fixture").unwrap();
                    assert_eq!(side.title.plain_text(), "Points");
                    assert_eq!(side.rows.len(), 4);
                    assert_eq!(side.rows[0].0.plain_text(), "[R] Alpha!");
                    assert!(side.rows[0]
                        .0
                        .runs
                        .iter()
                        .all(|r| r.style.color == Some(0xff5555)));
                    assert_eq!(side.rows[0].1.runs[0].style.color, Some(0xff5555));
                    assert_eq!(side.rows[1].0.plain_text(), "Beta!");
                    assert_eq!(side.rows[1].0.runs[0].style.color, Some(0x55ffff));
                    assert_eq!(side.rows[1].1.runs[0].style.color, Some(0xffaa00));
                    assert!(side.rows[1].1.runs[0].style.bold);
                    assert_eq!(side.rows[2].1.plain_text(), "");
                    assert_eq!(side.rows[3].1.plain_text(), "WIN");
                    let p = &state.teams["red"].parameters;
                    assert_eq!(
                        (p.visibility, p.collision, p.color, p.options),
                        (2, 3, Some(12), 2)
                    );
                }
                "team_update" => {
                    assert_eq!(state.team("Alpha").unwrap().parameters.color, None);
                    assert_eq!(
                        state.formatted_name("Alpha", None).plain_text(),
                        "[NEW] Alpha!"
                    );
                }
                "team_join" => {
                    let name =
                        state.formatted_name("beta", state.scores["main"]["beta"].display.as_ref());
                    assert_eq!(name.plain_text(), "[NEW] Beta!!");
                    assert_eq!(name.runs[1].style.color, Some(0x55ffff));
                }
                "team_leave" => assert!(state.team("Alpha").is_none()),
                "objective_update" => {
                    assert!(state.objectives["main"].hearts);
                    let side = state.sidebar("Fixture").unwrap();
                    assert_eq!(side.title.plain_text(), "Updated");
                    assert!(side.rows[0].1.runs.is_empty());
                    assert_eq!(side.rows[1].1.plain_text(), "9");
                }
                "reset_one" => assert!(!state.scores["main"].contains_key("beta")),
                "reset_all" => assert!(state.scores.values().all(|s| !s.contains_key("Alpha"))),
                "team_remove" => {
                    assert!(state.teams.is_empty());
                    assert!(state.membership.is_empty());
                }
                "objective_remove" => {
                    assert!(state.objectives.is_empty());
                    assert!(state.scores.is_empty());
                    assert!(state.slots.iter().all(Option::is_none));
                }
                _ => {}
            }
        }
    }
    #[test]
    fn every_original_packet_truncation_and_trailing_byte_leaves_state_unchanged() {
        let mut state = Store::default();
        for (label, id, bytes) in fixtures() {
            for end in 0..bytes.len() {
                let mut copy = state.clone();
                assert!(copy.packet(id, &bytes[..end]).is_err(), "{label} at {end}");
                assert_eq!(copy, state, "{label} at {end}");
            }
            let mut extra = bytes.clone();
            extra.push(0);
            let mut copy = state.clone();
            assert!(copy.packet(id, &extra).is_err());
            assert_eq!(copy, state);
            state.packet(id, &bytes).unwrap();
        }
    }
    #[test]
    fn top_fifteen_hide_internal_entries_and_use_case_insensitive_owner_ties() {
        let mut state = initial();
        let scores = state.scores.get_mut("main").unwrap();
        scores.clear();
        for (i, owner) in ["beta", "Alpha", "aardvark"].into_iter().enumerate() {
            scores.insert(
                owner.into(),
                Score {
                    value: 100,
                    display: Some(RichText::plain(&i.to_string())),
                    number_format: None,
                },
            );
        }
        for i in 0..30 {
            scores.insert(
                format!("p{i:02}"),
                Score {
                    value: i,
                    display: None,
                    number_format: None,
                },
            );
        }
        scores.insert(
            "#hidden".into(),
            Score {
                value: i32::MAX,
                display: None,
                number_format: None,
            },
        );
        let side = state.sidebar("unknown").unwrap();
        assert_eq!(side.rows.len(), 15);
        assert_eq!(side.rows[0].0.plain_text(), "2");
        assert_eq!(side.rows[1].0.plain_text(), "[R] 1!");
        assert_eq!(side.rows[2].0.plain_text(), "0");
        assert_eq!(side.rows[3].0.plain_text(), "p29");
        assert_eq!(side.rows[14].0.plain_text(), "p18");
    }
    #[test]
    fn team_color_selects_its_sidebar_and_falls_back_to_general_slot() {
        let mut state = initial();
        let mut other = state.objectives["main"].clone();
        other.title = RichText::plain("Team-specific");
        state.objectives.insert("other".into(), other);
        state.slots[15] = Some("other".into());
        assert_eq!(
            state.sidebar("Fixture").unwrap().title.plain_text(),
            "Team-specific"
        );
        assert_eq!(
            state.sidebar("unknown").unwrap().title.plain_text(),
            "Points"
        );
        state.slots[15] = None;
        assert_eq!(
            state.sidebar("Fixture").unwrap().title.plain_text(),
            "Points"
        );
    }
    #[test]
    fn membership_moves_remove_old_team_and_duplicate_objectives_are_rejected() {
        let mut state = initial();
        let packets = fixtures();
        let (_, id, data) = packets.iter().find(|p| p.0 == "team_add").unwrap();
        let mut tail = data.as_slice();
        string(&mut tail).unwrap();
        let mut moved = Vec::new();
        super::super::codec::string("blue", &mut moved);
        moved.extend(tail);
        state.packet(*id, &moved).unwrap();
        assert!(state.teams["red"].members.is_empty());
        assert!(std::ptr::eq(
            state.team("Alpha").unwrap(),
            &state.teams["blue"]
        ));
        let before = state.clone();
        assert!(state.packet(packets[0].1, &packets[0].2).is_err());
        assert_eq!(state, before);
    }
}
