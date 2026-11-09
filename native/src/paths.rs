use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
pub fn config_root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}
fn expand(s: &str) -> PathBuf {
    s.strip_prefix("~/")
        .map_or_else(|| PathBuf::from(s), |v| home().join(v))
}
pub fn data_file(notes: bool) -> PathBuf {
    if let Some(p) = std::env::var_os(if notes {
        "STICKY_NOTES_FILE"
    } else {
        "PRIORITIES_FILE"
    }) {
        return PathBuf::from(p);
    }
    let config = config_root()
        .join(if notes { "StickyNotes" } else { "Pin" })
        .join("settings.json");
    if let Ok(s) = std::fs::read_to_string(config) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(path) = value
                .get("file")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                return expand(path);
            }
        }
    }
    let documents = std::fs::read_to_string(config_root().join("user-dirs.dirs"))
        .ok()
        .and_then(|s| {
            s.lines().find_map(|line| {
                line.strip_prefix("XDG_DOCUMENTS_DIR=").map(|v| {
                    v.trim_matches('"')
                        .replace("$HOME", &home().to_string_lossy())
                })
            })
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("Documents"));
    documents.join(if notes { "Sticky Notes.txt" } else { "Pin.md" })
}
