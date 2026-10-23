//! Client text components: inherited styles, legacy codes and styled line layout.
use anyhow::{ensure, Result};
use serde_json::Value;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub obfuscated: bool,
    pub shadow: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: Style,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichText {
    pub runs: Vec<Run>,
}
impl From<&str> for RichText {
    fn from(value: &str) -> Self {
        Self::plain(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyledChar {
    pub ch: char,
    pub style: Style,
}
const COLORS: [u32; 16] = [
    0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa, 0x555555,
    0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
];
const NAMES: [&str; 16] = [
    "black",
    "dark_blue",
    "dark_green",
    "dark_aqua",
    "dark_red",
    "dark_purple",
    "gold",
    "gray",
    "dark_gray",
    "blue",
    "green",
    "aqua",
    "red",
    "light_purple",
    "yellow",
    "white",
];
pub fn color(name: &str) -> Option<u32> {
    if let Some(hex) = name.strip_prefix('#') {
        if hex.len() == 6 {
            return u32::from_str_radix(hex, 16).ok();
        }
    }
    NAMES.iter().position(|&n| n == name).map(|i| COLORS[i])
}
impl RichText {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.runs.last_mut().filter(|r| r.style == style) {
            last.text.push_str(text);
        } else {
            self.runs.push(Run {
                text: text.into(),
                style,
            });
        }
    }
    fn legacy_into(&mut self, text: &str, base: Style) {
        let mut style = base;
        let mut chars = text.chars();
        let mut pending = String::new();
        while let Some(c) = chars.next() {
            if c == '§' {
                if let Some(code) = chars.next() {
                    self.push(&pending, style);
                    pending.clear();
                    let code = code.to_ascii_lowercase();
                    if let Some(n) = code.to_digit(16) {
                        style = Style {
                            color: Some(COLORS[n as usize]),
                            shadow: style.shadow,
                            ..Default::default()
                        };
                    } else {
                        match code {
                            'k' => style.obfuscated = true,
                            'l' => style.bold = true,
                            'm' => style.strike = true,
                            'n' => style.underline = true,
                            'o' => style.italic = true,
                            'r' => style = base,
                            _ => {}
                        }
                    }
                }
            } else {
                pending.push(c);
            }
        }
        self.push(&pending, style);
    }
    pub fn plain(text: &str) -> Self {
        let mut out = Self::default();
        out.legacy_into(text, Style::default());
        out
    }
    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
    pub fn from_json(value: &Value) -> Result<Self> {
        let mut out = Self::default();
        out.component(value, Style::default(), 0)?;
        ensure!(
            out.runs
                .iter()
                .map(|r| r.text.chars().count())
                .sum::<usize>()
                <= 32767,
            "Text component too long"
        );
        Ok(out)
    }
    fn component(&mut self, v: &Value, base: Style, depth: usize) -> Result<()> {
        ensure!(depth <= 128, "Text component too deep");
        match v {
            Value::String(s) => self.legacy_into(s, base),
            Value::Array(values) => {
                // A component array appends later components to its first root.
                let inherited = values.first().map(|v| apply_style(v, base)).unwrap_or(base);
                for (i, v) in values.iter().enumerate() {
                    self.component(v, if i == 0 { base } else { inherited }, depth + 1)?;
                }
            }
            Value::Object(_) => {
                let style = apply_style(v, base);
                if let Some(text) = v.get("text").and_then(Value::as_str) {
                    self.legacy_into(text, style);
                } else if let Some(key) = v.get("translate").and_then(Value::as_str) {
                    let template = translation(key)
                        .or_else(|| v.get("fallback").and_then(Value::as_str).map(str::to_owned))
                        .unwrap_or_else(|| key.into());
                    let args = v
                        .get("with")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    self.translate(&template, args, style, depth + 1)?;
                } else if let Some(key) = v
                    .get("keybind")
                    .or(v.get("selector"))
                    .and_then(Value::as_str)
                {
                    self.legacy_into(key, style);
                }
                if let Some(extra) = v.get("extra").and_then(Value::as_array) {
                    for child in extra {
                        self.component(child, style, depth + 1)?;
                    }
                }
            }
            Value::Number(n) => self.push(&n.to_string(), base),
            Value::Bool(b) => self.push(if *b { "true" } else { "false" }, base),
            _ => {}
        }
        ensure!(self.runs.len() <= 32767, "Too many text spans");
        Ok(())
    }
    fn translate(
        &mut self,
        template: &str,
        args: &[Value],
        style: Style,
        depth: usize,
    ) -> Result<()> {
        let mut chars = template.chars().peekable();
        let mut text = String::new();
        let mut next = 0;
        while let Some(c) = chars.next() {
            if c != '%' {
                text.push(c);
                continue;
            }
            if chars.peek() == Some(&'%') {
                chars.next();
                text.push('%');
                continue;
            }
            let mut digits = String::new();
            while chars.peek().is_some_and(char::is_ascii_digit) {
                digits.push(chars.next().unwrap());
            }
            let index = if digits.is_empty() {
                let i = next;
                next += 1;
                i
            } else {
                if chars.next() != Some('$') {
                    text.push('%');
                    text.push_str(&digits);
                    continue;
                }
                digits.parse::<usize>().unwrap_or(0).saturating_sub(1)
            };
            if chars.next() == Some('s') {
                self.legacy_into(&text, style);
                text.clear();
                if let Some(arg) = args.get(index) {
                    self.component(arg, style, depth + 1)?;
                } else {
                    self.push("%s", style);
                }
            } else {
                text.push('%');
            }
        }
        self.legacy_into(&text, style);
        Ok(())
    }
    pub fn chars(&self) -> Vec<StyledChar> {
        self.runs
            .iter()
            .flat_map(|r| {
                r.text
                    .chars()
                    .map(move |ch| StyledChar { ch, style: r.style })
            })
            .collect()
    }
    pub fn lines(&self, width: f32, measure: impl Fn(StyledChar) -> f32) -> Vec<Vec<StyledChar>> {
        let mut lines = Vec::new();
        let mut line: Vec<StyledChar> = Vec::new();
        let mut size = 0.;
        for c in self.chars() {
            if c.ch == '\r' {
                continue;
            }
            if c.ch == '\n' {
                lines.push(std::mem::take(&mut line));
                size = 0.;
                continue;
            }
            let advance = measure(c);
            if !line.is_empty() && size + advance > width {
                if c.ch == ' ' {
                    lines.push(std::mem::take(&mut line));
                    size = 0.;
                    continue;
                }
                if let Some(space) = line.iter().rposition(|c| c.ch == ' ') {
                    let tail = line.split_off(space + 1);
                    line.pop();
                    lines.push(std::mem::replace(&mut line, tail));
                    size = line.iter().copied().map(&measure).sum();
                } else {
                    lines.push(std::mem::take(&mut line));
                    size = 0.;
                }
            }
            line.push(c);
            size += advance;
        }
        lines.push(line);
        lines
    }
}
pub(crate) fn apply_style(v: &Value, mut style: Style) -> Style {
    if let Some(name) = v.get("color").and_then(Value::as_str) {
        style.color = color(name).or(style.color);
    }
    for (key, flag) in [
        ("bold", &mut style.bold),
        ("italic", &mut style.italic),
        ("underlined", &mut style.underline),
        ("strikethrough", &mut style.strike),
        ("obfuscated", &mut style.obfuscated),
    ] {
        if let Some(v) = v.get(key) {
            if let Some(value) = v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)) {
                *flag = value;
            }
        }
    }
    if let Some(shadow) = v.get("shadow_color").and_then(Value::as_i64) {
        style.shadow = Some(shadow as u32);
    }
    style
}
static LANGUAGE: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
    std::sync::OnceLock::new();
pub fn localized(key: &str) -> Option<&'static str> {
    LANGUAGE.get()?.get(key).map(String::as_str)
}
fn translation(key: &str) -> Option<String> {
    LANGUAGE.get().and_then(|map| map.get(key)).cloned()
}
pub fn load_language(cache: &std::path::Path) {
    LANGUAGE.get_or_init(|| {
        let dir = crate::assets::root(cache);
        let mut map: std::collections::BTreeMap<String, String> =
            std::fs::read(dir.join("resources/assets/minecraft/lang/en_us.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
        if let Ok(bytes) = std::fs::read(dir.join("asset-index.json")) {
            if let Ok(index) = serde_json::from_slice::<Value>(&bytes) {
                if let Some(hash) = index["objects"]["minecraft/lang/ru_ru.json"]["hash"]
                    .as_str()
                    .filter(|h| h.len() == 40 && h.bytes().all(|c| c.is_ascii_hexdigit()))
                {
                    let path = cache.join("objects").join(&hash[..2]).join(hash);
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Ok(ru) = serde_json::from_slice::<
                            std::collections::BTreeMap<String, String>,
                        >(&bytes)
                        {
                            map.extend(ru);
                        }
                    }
                }
            }
        }
        map
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_extra_styles_and_false_override_match_components() {
        let t=RichText::from_json(&serde_json::json!({"text":"Lucky","color":"dark_green","bold":true,"extra":[{"text":"World","color":"red","bold":false},{"text":"!"}]})).unwrap();
        assert_eq!(t.plain_text(), "LuckyWorld!");
        assert_eq!(t.runs[0].style.color, Some(0x00aa00));
        assert!(t.runs[0].style.bold);
        assert!(!t.runs[1].style.bold);
        assert!(t.runs[2].style.bold);
    }
    #[test]
    fn legacy_color_clears_decorations_and_reset_restores_component_style() {
        let t = RichText::from_json(
            &serde_json::json!({"text":"A§cB§lC§rD","bold":true,"color":"gold"}),
        )
        .unwrap();
        assert_eq!(t.plain_text(), "ABCD");
        assert!(!t.runs[1].style.bold);
        assert!(t.runs[2].style.bold);
        assert_eq!(t.runs[3].style.color, Some(0xffaa00));
    }
    #[test]
    fn wrapped_lines_preserve_styles_and_explicit_empty_paragraphs() {
        let t=RichText::from_json(&serde_json::json!({"text":"Ошибка\n\n","extra":[{"text":"Повторите позже","color":"red"}]})).unwrap();
        let lines = t.lines(9., |_| 1.);
        assert_eq!(
            lines
                .iter()
                .map(|l| l.iter().map(|c| c.ch).collect::<String>())
                .collect::<Vec<_>>(),
            ["Ошибка", "", "Повторите", "позже"]
        );
        assert_eq!(lines[3][0].style.color, Some(0xff5555));
    }
    #[test]
    fn indexed_translations_preserve_argument_styles() {
        let t=RichText::from_json(&serde_json::json!({"translate":"unknown.test","fallback":"%2$s: %1$s %%","with":[{"text":"Ошибка","color":"red"},"Сервер"]})).unwrap();
        assert_eq!(t.plain_text(), "Сервер: Ошибка %");
        assert!(t
            .runs
            .iter()
            .any(|r| r.text == "Ошибка" && r.style.color == Some(0xff5555)));
    }
}
