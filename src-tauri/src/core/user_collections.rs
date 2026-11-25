//! Collections you add yourself (fork addition): GitHub repositories whose
//! skills join the Collections tab next to the ones shipped with the app.
//!
//! Stored in `<library>/collections.json`, so they survive app updates and
//! move with the library. Fetching a repo costs two GitHub API calls (repo
//! info + recursive tree); file contents come from raw.githubusercontent.com,
//! which does not use API quota. Nothing is re-fetched unless asked.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FILE: &str = "collections.json";
pub const SHARE_FORMAT: &str = "skills-manager-collections";
/// Upper bound on skills read from one repository.
const MAX_SKILLS: usize = 1500;
const MAX_DESCRIPTION: usize = 400;
const FETCH_THREADS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionSkill {
    /// Folder holding SKILL.md; "" when the repo root is the skill.
    pub path: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserCollection {
    /// `owner/repo`
    pub repo: String,
    pub title: String,
    pub branch: String,
    /// Unix ms of the last fetch.
    pub fetched_at: i64,
    pub skills: Vec<CollectionSkill>,
}

pub fn store_path() -> PathBuf {
    super::central_repo::base_dir().join(FILE)
}

pub fn load(path: &Path) -> Vec<UserCollection> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Write-then-rename, so a crash never leaves half a file.
pub fn save(path: &Path, collections: &[UserCollection]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.part");
    std::fs::write(&tmp, serde_json::to_string_pretty(collections)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// `owner/repo` from what a user pastes: the slug itself, a github.com URL
/// (with or without `.git`, `/tree/...`), or an SSH remote.
pub fn parse_repo(input: &str) -> Result<String> {
    let s = input.trim().trim_end_matches('/');
    let rest = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
        .or_else(|| s.strip_prefix("github.com/"))
        .or_else(|| s.strip_prefix("git@github.com:"))
        .unwrap_or(s);
    let mut parts = rest.split('/');
    let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else {
        bail!("Enter a GitHub repository as owner/repo or its URL");
    };
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let ok = |p: &str| {
        !p.is_empty()
            && !p.starts_with('.')
            && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !ok(owner) || !ok(repo) || s.contains("://") && !s.contains("github.com") {
        bail!("Only GitHub repositories can be added as collections (owner/repo)");
    }
    Ok(format!("{owner}/{repo}"))
}

/// One entry per name + description; repos often ship the same skill once
/// per agent folder. Keeps the shortest path, ties broken alphabetically.
pub fn dedupe(skills: Vec<CollectionSkill>) -> Vec<CollectionSkill> {
    let mut best: HashMap<(String, String), CollectionSkill> = HashMap::new();
    for s in skills {
        let key = (s.name.to_lowercase(), s.description.clone());
        match best.get(&key) {
            Some(have) if (have.path.len(), &have.path) <= (s.path.len(), &s.path) => {}
            _ => {
                best.insert(key, s);
            }
        }
    }
    let mut out: Vec<_> = best.into_values().collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
    out
}

fn skill_from(path: &str, repo: &str, raw: &str) -> CollectionSkill {
    let meta = super::skill_metadata::parse_frontmatter(raw);
    let folder = path.rsplit('/').next().filter(|f| !f.is_empty()).unwrap_or_else(|| {
        repo.split('/').nth(1).unwrap_or(repo)
    });
    CollectionSkill {
        path: path.to_string(),
        name: meta.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| folder.to_string()),
        description: meta
            .description
            .unwrap_or_default()
            .trim()
            .chars()
            .take(MAX_DESCRIPTION)
            .collect(),
    }
}

/// Read every SKILL.md in `repo` (its default branch).
pub fn fetch(client: &reqwest::blocking::Client, repo: &str) -> Result<(String, Vec<CollectionSkill>)> {
    let get_json = |url: &str| -> Result<serde_json::Value> {
        let res = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .with_context(|| format!("Cannot reach GitHub for {repo}"))?;
        match res.status().as_u16() {
            200 => Ok(res.json()?),
            404 => Err(anyhow!("{repo} was not found on GitHub (is it private or renamed?)")),
            403 | 429 => Err(anyhow!("GitHub rate limit reached — try again in a few minutes")),
            code => Err(anyhow!("GitHub returned HTTP {code} for {repo}")),
        }
    };
    let info = get_json(&format!("https://api.github.com/repos/{repo}"))?;
    let branch = info["default_branch"].as_str().unwrap_or("main").to_string();
    let tree = get_json(&format!(
        "https://api.github.com/repos/{repo}/git/trees/{branch}?recursive=1"
    ))?;
    let files: Vec<String> = tree["tree"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter(|e| e["type"] == "blob")
                .filter_map(|e| e["path"].as_str())
                .filter(|p| {
                    let lower = p.to_lowercase();
                    (lower == "skill.md" || lower.ends_with("/skill.md"))
                        && !p.split('/').any(|seg| seg == "node_modules" || seg == ".git")
                })
                .take(MAX_SKILLS)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        bail!("{repo} has no SKILL.md files");
    }

    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..FETCH_THREADS.min(files.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(file) = files.get(i) else { break };
                let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/{file}");
                let Ok(res) = client.get(&url).send() else { continue };
                if !res.status().is_success() {
                    continue;
                }
                let Ok(raw) = res.text() else { continue };
                let dir = file.rfind('/').map(|i| &file[..i]).unwrap_or("");
                results.lock().unwrap().push(skill_from(dir, repo, &raw));
            });
        }
    });
    let skills = dedupe(results.into_inner().unwrap());
    if skills.is_empty() {
        bail!("Couldn't read any SKILL.md in {repo}");
    }
    Ok((branch, skills))
}

/// The file a collections share writes: repos and titles only — the
/// receiver fetches the skills themselves.
#[derive(Debug, Serialize, Deserialize)]
struct ShareFile {
    format: String,
    version: u32,
    collections: Vec<SharedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharedEntry {
    pub repo: String,
    pub title: String,
}

pub fn to_share(collections: &[UserCollection]) -> String {
    let file = ShareFile {
        format: SHARE_FORMAT.into(),
        version: 1,
        collections: collections
            .iter()
            .map(|c| SharedEntry { repo: c.repo.clone(), title: c.title.clone() })
            .collect(),
    };
    serde_json::to_string_pretty(&file).unwrap_or_default() + "\n"
}

/// Read a collections share: our JSON format, or a plain list of repos
/// (one `owner/repo` or URL per line, `#` comments allowed).
pub fn from_share(raw: &str) -> Result<Vec<SharedEntry>> {
    if raw.trim_start().starts_with('{') {
        let file: ShareFile = serde_json::from_str(raw).context("Not a collections file")?;
        if file.format != SHARE_FORMAT {
            bail!("Not a collections file");
        }
        return file
            .collections
            .into_iter()
            .map(|e| Ok(SharedEntry { repo: parse_repo(&e.repo)?, title: e.title }))
            .collect();
    }
    let entries: Vec<SharedEntry> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let repo = parse_repo(l)?;
            Ok(SharedEntry { title: repo.split('/').nth(1).unwrap_or(&repo).to_string(), repo })
        })
        .collect::<Result<_>>()?;
    if entries.is_empty() {
        bail!("No repositories found in that file");
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(path: &str, name: &str, description: &str) -> CollectionSkill {
        CollectionSkill { path: path.into(), name: name.into(), description: description.into() }
    }

    #[test]
    fn parses_slugs_urls_and_ssh_remotes() {
        for input in [
            "acme/skills",
            "https://github.com/acme/skills",
            "https://github.com/acme/skills.git",
            "https://github.com/acme/skills/tree/main/x",
            "git@github.com:acme/skills.git",
            "  github.com/acme/skills/  ",
        ] {
            assert_eq!(parse_repo(input).unwrap(), "acme/skills", "{input}");
        }
        for bad in ["", "acme", "https://gitlab.com/acme/skills", "../x/y", "acme/sk ills"] {
            assert!(parse_repo(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn dedupe_keeps_the_shortest_copy_per_name_and_description() {
        let kept = dedupe(vec![
            skill("providers/claude/skills/pdf", "pdf", "Read PDFs"),
            skill("skills/pdf", "PDF", "Read PDFs"),
            skill("other/pdf", "pdf", "A different pdf skill"),
        ]);
        let paths: Vec<&str> = kept.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, vec!["other/pdf", "skills/pdf"]);
    }

    #[test]
    fn reads_name_and_description_with_folder_fallback() {
        let s = skill_from("skills/pdf", "acme/skills", "---\nname: pdf-tools\ndescription: Fill forms\n---\n");
        assert_eq!((s.name.as_str(), s.description.as_str()), ("pdf-tools", "Fill forms"));
        let bare = skill_from("skills/pdf", "acme/skills", "# no frontmatter");
        assert_eq!(bare.name, "pdf");
        let root = skill_from("", "acme/solo", "# root skill");
        assert_eq!(root.name, "solo");
    }

    #[test]
    fn store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        assert!(load(&path).is_empty());
        let c = UserCollection {
            repo: "acme/skills".into(),
            title: "Acme".into(),
            branch: "main".into(),
            fetched_at: 1,
            skills: vec![skill("skills/pdf", "pdf", "x")],
        };
        save(&path, std::slice::from_ref(&c)).unwrap();
        assert_eq!(load(&path), vec![c]);
    }

    #[test]
    fn share_round_trips_and_accepts_plain_lists() {
        let c = UserCollection {
            repo: "acme/skills".into(),
            title: "Acme".into(),
            branch: "main".into(),
            fetched_at: 1,
            skills: vec![],
        };
        let shared = from_share(&to_share(&[c])).unwrap();
        assert_eq!(shared, vec![SharedEntry { repo: "acme/skills".into(), title: "Acme".into() }]);

        let plain = from_share("# my list\nacme/skills\nhttps://github.com/foo/bar.git\n\n").unwrap();
        assert_eq!(plain.iter().map(|e| e.repo.as_str()).collect::<Vec<_>>(), vec!["acme/skills", "foo/bar"]);
        assert!(from_share("{\"format\":\"nope\",\"version\":1,\"collections\":[]}").is_err());
        assert!(from_share("\n# nothing\n").is_err());
    }

    /// Hits GitHub: `cargo test fetch_real_repo -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fetch_real_repo() {
        let client = crate::core::skillssh_api::build_http_client(None, 30);
        let (branch, skills) = fetch(&client, "anthropics/skills").unwrap();
        println!("{branch}: {} skills, e.g. {:?}", skills.len(), skills.first());
        assert!(skills.len() > 5);
        assert!(skills.iter().filter(|s| !s.description.is_empty()).count() > skills.len() / 2);
    }
}
