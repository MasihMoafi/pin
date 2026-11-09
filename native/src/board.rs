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
    pub fn index(&self, id: u64) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    pub fn end(&self, i: usize) -> usize {
        (i + 1..self.tasks.len())
            .find(|&j| self.tasks[j].depth <= self.tasks[i].depth)
            .unwrap_or(self.tasks.len())
    }

    pub fn parent(&self, i: usize) -> Option<usize> {
        (0..i)
            .rev()
            .find(|&j| self.tasks[j].depth < self.tasks[i].depth)
    }

    pub fn siblings(&self, i: usize) -> Vec<usize> {
        let start = self.parent(i).map_or(0, |p| p + 1);
        let end = self.parent(i).map_or(self.tasks.len(), |p| self.end(p));
        (start..end)
            .filter(|&j| self.tasks[j].depth == self.tasks[i].depth)
            .collect()
    }

    pub fn completed_last(&mut self) {
        fn sort(tasks: &[Task]) -> Vec<Task> {
            let mut groups: Vec<Vec<Task>> = Vec::new();
            for task in tasks {
                if groups.is_empty() || task.depth == tasks[0].depth {
                    groups.push(Vec::new());
                }
                groups.last_mut().unwrap().push(task.clone());
            }
            groups.sort_by_key(|g| g[0].done);
            groups
                .into_iter()
                .flat_map(|g| {
                    let mut result = vec![g[0].clone()];
                    result.extend(sort(&g[1..]));
                    result
                })
                .collect()
        }
        self.tasks = sort(&self.tasks);
    }

    pub fn toggle_done(&mut self, id: u64, today: &str) {
        let Some(i) = self.index(id) else {
            return;
        };
        self.tasks[i].done = !self.tasks[i].done;
        self.tasks[i].completed = if self.tasks[i].daily && self.tasks[i].done {
            Some(today.to_owned())
        } else {
            None
        };
        if self.tasks[i].done {
            let end = self.end(i);
            let target = self.parent(i).map_or(self.tasks.len(), |p| self.end(p));
            let branch: Vec<_> = self.tasks.drain(i..end).collect();
            self.tasks
                .splice(target - branch.len()..target - branch.len(), branch);
        }
        self.completed_last();
    }

    pub fn rollover(&mut self, today: &str) -> bool {
        let mut changed = false;
        for task in &mut self.tasks {
            if task.daily && task.done && task.completed.as_deref().is_some_and(|d| d < today) {
                task.done = false;
                task.completed = None;
                if let Some(date) = &mut task.date {
                    if date.get(..10).is_some_and(|day| day < today) {
                        date.replace_range(..10, today);
                    }
                }
                changed = true;
            } else if task.daily && task.done && task.completed.is_none() {
                task.completed = Some(today.to_owned());
                changed = true;
            }
        }
        if changed {
            self.completed_last();
        }
        changed
    }

    pub fn move_branch(&mut self, id: u64, target: u64, after: bool) -> bool {
        let (Some(i), Some(j)) = (self.index(id), self.index(target)) else {
            return false;
        };
        let end = self.end(i);
        if (i..end).contains(&j) {
            return false;
        }
        let depth = self.tasks[j].depth as isize - self.tasks[i].depth as isize;
        let dest = if after { self.end(j) } else { j };
        let mut branch: Vec<_> = self.tasks.drain(i..end).collect();
        for task in &mut branch {
            task.depth = (task.depth as isize + depth) as usize;
        }
        let dest = if dest >= end {
            dest - branch.len()
        } else {
            dest
        };
        self.tasks.splice(dest..dest, branch);
        self.completed_last();
        true
    }

    pub fn indent(&mut self, id: u64, out: bool) -> bool {
        let Some(i) = self.index(id) else {
            return false;
        };
        let end = self.end(i);
        if out {
            let Some(parent) = self.parent(i) else {
                return false;
            };
            let dest = self.end(parent);
            let mut branch: Vec<_> = self.tasks.drain(i..end).collect();
            for task in &mut branch {
                task.depth -= 1;
            }
            self.tasks
                .splice(dest - branch.len()..dest - branch.len(), branch);
        } else {
            let siblings = self.siblings(i);
            if siblings.first() == Some(&i) {
                return false;
            }
            for task in &mut self.tasks[i..end] {
                task.depth += 1;
            }
        }
        self.completed_last();
        true
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
    fn daily_resets_once_on_next_local_day_and_survives_restart() {
        let mut b = Board::parse("- [ ] Daily 📅 2026-11-09 09:30 🔁 daily\n- [x] One-off\n");
        let id = b.tasks[0].id;
        b.toggle_done(id, "2026-11-09");
        let mut b = Board::parse(&b.markdown());
        assert!(!b.rollover("2026-11-09"));
        assert!(b.rollover("2026-11-10"));
        assert!(!b.tasks[0].done);
        assert_eq!(b.tasks[0].date.as_deref(), Some("2026-11-10 09:30"));
        assert!(b.tasks[1].done);
        assert!(!b.rollover("2026-11-10"));
    }

    #[test]
    fn finishing_moves_whole_branch_last_and_history_can_restore_it() {
        let mut b = Board::parse("- [ ] First\n  - [ ] Child\n- [x] Older done\n- [ ] Second\n");
        let before = b.clone();
        b.toggle_done(1, "2026-11-09");
        assert_eq!(
            b.tasks.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["Second", "Older done", "First", "Child"]
        );
        b = before;
        assert_eq!(b.tasks[0].text, "First");
        assert!(!b.tasks[0].done);
    }

    #[test]
    fn moving_and_outdenting_keeps_children_and_rejects_self_drop() {
        let mut b = Board::parse(
            "- [ ] Parent\n  - [ ] Child\n    - [ ] Grandchild\n  - [ ] Other\n- [ ] Last\n",
        );
        assert!(!b.move_branch(1, 3, true));
        assert!(b.indent(2, true));
        assert_eq!(
            b.tasks
                .iter()
                .map(|t| (&*t.text, t.depth))
                .collect::<Vec<_>>(),
            [
                ("Parent", 0),
                ("Other", 1),
                ("Child", 0),
                ("Grandchild", 1),
                ("Last", 0)
            ]
        );
        assert!(b.move_branch(2, 5, true));
        assert_eq!(b.tasks[3].text, "Child");
        assert_eq!(b.tasks[4].text, "Grandchild");
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
