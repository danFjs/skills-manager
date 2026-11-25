//! Creating and editing skills in the library (fork addition).

/// Names Windows refuses as file or folder names.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Largest SKILL.md the editor will write.
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// A new skill's folder name: what agents and `npx skills` expect — short,
/// lowercase-friendly, and safe on every OS.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.trim() != name {
        return Err("Give the skill a name without leading or trailing spaces".into());
    }
    if name.chars().count() > 64 {
        return Err("Skill names can be at most 64 characters".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err("Use only letters, digits, '-', '_' and '.' in the skill name".into());
    }
    if name.starts_with('.') {
        return Err("Skill names can't start with a dot".into());
    }
    if WINDOWS_RESERVED_NAMES.contains(&name.to_ascii_uppercase().as_str()) {
        return Err(format!("'{name}' is a reserved name on Windows"));
    }
    Ok(())
}

/// YAML double-quoted scalar, so any description (colons, quotes,
/// newlines) round-trips through frontmatter parsers.
fn yaml_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The starting SKILL.md for a new skill.
pub fn new_skill_md(name: &str, description: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: {}\n---\n\n# {name}\n\n\
         ## When to use\n\n- \n\n## Instructions\n\n1. \n",
        yaml_quoted(description.trim())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::skill_metadata;

    #[test]
    fn accepts_plain_names_and_rejects_unsafe_ones() {
        for ok in ["pdf", "git-helper", "my_skill.v2"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", " pdf", "a/b", "../x", ".hidden", "con", "has space", &"x".repeat(65)] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn template_frontmatter_round_trips_awkward_descriptions() {
        let tmp = tempfile::tempdir().unwrap();
        let description = "Use when: \"quoted\" things\nspan lines \\ backslashes";
        std::fs::write(tmp.path().join("SKILL.md"), new_skill_md("demo", description)).unwrap();
        let meta = skill_metadata::parse_skill_md(tmp.path());
        assert_eq!(meta.name.as_deref(), Some("demo"));
        assert_eq!(meta.description.as_deref(), Some(description));
    }
}
