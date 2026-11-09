use chrono::NaiveDate;
use regex::Regex;
use std::sync::LazyLock;

pub const COLORS: [(&str, &str); 12] = [
    ("🟢", "#22c55e"),
    ("🔵", "#3b82f6"),
    ("🟣", "#a855f7"),
    ("🟠", "#f97316"),
    ("🟡", "#eab308"),
    ("🔴", "#ef4444"),
    ("#ec4899", "#ec4899"),
    ("#14b8a6", "#14b8a6"),
    ("#06b6d4", "#06b6d4"),
    ("#84cc16", "#84cc16"),
    ("#a16207", "#a16207"),
    ("#64748b", "#64748b"),
];

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub depth: usize,
    pub text: String,
    pub color: Option<String>,
    pub date: Option<String>,
    pub done: bool,
    pub daily: bool,
    pub completed: Option<String>,
}

impl Task {
    pub fn blank(id: u64, depth: usize, color: Option<String>) -> Self {
        Self {
            id,
            depth,
            text: String::new(),
            color,
            date: None,
            done: false,
            daily: false,
            completed: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Board {
    pub preamble: String,
    pub tasks: Vec<Task>,
}

static LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)[-*] (?:\[([ xX])\] )?(.*)$").unwrap());
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*📅\s?(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}))?\s*$").unwrap());
static DAILY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+🔁 daily(?: ✅ (\d{4}-\d{2}-\d{2}))?\s*$").unwrap());
static HEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"^<mark style="background: (#[0-9a-fA-F]{6})(?:[0-9a-fA-F]{2})?;">(.*)</mark>$"##)
        .unwrap()
});

impl Board {
    pub fn parse(md: &str) -> Self {
        let mut board = Self::default();
        let mut preamble = Vec::new();
        for line in md.lines() {
            let Some(c) = LINE.captures(line) else {
                if board.tasks.is_empty() && !line.trim().is_empty() {
                    preamble.push(line);
                }
                continue;
            };
            let depth = c[1].replace('\t', "  ").len() / 2;
            let mut body = c[3].to_owned();
            let done = c
                .get(2)
                .is_some_and(|v| v.as_str().eq_ignore_ascii_case("x"));
            let mut task = Task::blank(board.tasks.len() as u64 + 1, depth, None);
            task.done = done;
            if let Some(d) = DAILY.captures(&body) {
                task.daily = true;
                task.completed = d.get(1).map(|v| v.as_str().to_owned());
                body.truncate(d.get(0).unwrap().start());
            }
            if let Some(d) = DATE.captures(&body) {
                task.date = Some(match d.get(2) {
                    Some(t) => format!("{} {}", &d[1], t.as_str()),
                    None => d[1].to_owned(),
                });
                body.truncate(d.get(0).unwrap().start());
            }
            if let Some(inner) = body.strip_prefix("==").and_then(|v| v.strip_suffix("==")) {
                if let Some((marker, _)) = COLORS.iter().take(6).find(|(m, _)| inner.starts_with(m))
                {
                    task.color = Some((*marker).to_owned());
                    body = inner[marker.len()..].to_owned();
                }
            } else if let Some(c) = HEX.captures(&body) {
                task.color = Some(c[1].to_lowercase());
                body = c[2].to_owned();
            }
            task.text = body.trim().to_owned();
            // Malformed indentation cannot create an orphan tree.
            task.depth = board
                .tasks
                .last()
                .map_or(0, |prev| depth.min(prev.depth + 1));
            board.tasks.push(task);
        }
        board.preamble = preamble.join("\n");
        board
    }

    pub fn markdown(&self) -> String {
        let mut md = String::new();
        if !self.preamble.is_empty() {
            md.push_str(&self.preamble);
            md.push_str("\n\n");
        }
        for task in &self.tasks {
            let mut body = task.text.clone();
            if let Some(color) = &task.color {
                body = if color.starts_with('#') {
                    format!("<mark style=\"background: {color};\">{body}</mark>")
                } else {
                    format!("=={color}{body}==")
                };
            }
            if let Some(date) = &task.date {
                body.push_str(&format!(" 📅 {date}"));
            }
            if task.daily {
                body.push_str(" 🔁 daily");
                if let Some(day) = &task.completed {
                    body.push_str(&format!(" ✅ {day}"));
                }
            }
            md.push_str(&format!(
                "{}- [{}] {body}\n",
                "  ".repeat(task.depth),
                if task.done { 'x' } else { ' ' }
            ));
        }
        md
    }
}

pub fn valid_date(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    match s.split_once(' ') {
        Some((d, t)) => {
            NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == d)
                && chrono::NaiveTime::parse_from_str(t, "%H:%M")
                    .is_ok_and(|time| time.format("%H:%M").to_string() == t)
        }
        None => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_markdown_and_daily_round_trip() {
        let md = "Intro.\n\n- [ ] ==🟢Priority== 📅 2026-11-09 09:30 🔁 daily\n  - [x] <mark style=\"background: #ec4899;\">Child</mark>\n- [x] Repeat ✅ not metadata 🔁 daily ✅ 2026-11-09\n";
        assert_eq!(Board::parse(md).markdown(), md);
    }

    #[test]
    fn dates_are_validated_including_leap_days() {
        assert!(valid_date("2028-02-29 23:59"));
        assert!(!valid_date("2026-02-29"));
        assert!(!valid_date("2026-11-09 24:00"));
        assert!(!valid_date("2026-11-9"));
        assert!(!valid_date("2026-11-09 9:00"));
    }
}
