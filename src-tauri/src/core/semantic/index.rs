//! Full-text index of library skills and ranked search over it.

use super::query::{match_expr, parse, MARK_END, MARK_START};
use super::text_hash;
use crate::core::skill_store::SkillStore;
use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// What gets indexed for one library skill.
#[derive(Debug, Clone)]
pub struct Doc {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
    pub tags: Vec<String>,
    /// `source_type` plus `source_ref`, e.g. `git https://github.com/o/r`.
    pub source: String,
}

impl Doc {
    fn hash(&self) -> String {
        text_hash(&format!(
            "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
            self.name,
            self.description,
            self.body,
            self.tags.join(","),
            self.source
        ))
    }
}

/// Characters of SKILL.md indexed per skill — enough for every real one.
const MAX_BODY_CHARS: usize = 200_000;

fn read_body(dir: &Path) -> String {
    for name in ["SKILL.md", "skill.md"] {
        if let Ok(raw) = std::fs::read_to_string(dir.join(name)) {
            return raw.chars().take(MAX_BODY_CHARS).collect();
        }
    }
    String::new()
}

/// Every library skill as an indexable document.
pub fn docs_from_store(store: &SkillStore) -> anyhow::Result<Vec<Doc>> {
    let tags = store.get_tags_map().unwrap_or_default();
    Ok(store
        .get_all_skills()?
        .into_iter()
        .map(|s| Doc {
            body: read_body(Path::new(&s.central_path)),
            tags: tags.get(&s.id).cloned().unwrap_or_default(),
            source: format!("{} {}", s.source_type, s.source_ref.unwrap_or_default())
                .trim()
                .to_string(),
            description: s.description.unwrap_or_default(),
            name: s.name,
            id: s.id,
        })
        .collect())
}

/// Bring the index in line with `docs`: re-index changed skills, drop
/// removed ones. Returns how many rows changed.
pub fn sync(conn: &mut Connection, docs: &[Doc]) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    let mut changed = 0;
    let keep: HashSet<&str> = docs.iter().map(|d| d.id.as_str()).collect();

    let existing: Vec<(i64, String)> = {
        let mut stmt = tx.prepare("SELECT rid, id FROM docs")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (rid, id) in existing {
        if !keep.contains(id.as_str()) {
            tx.execute("DELETE FROM docs_fts WHERE rowid = ?1", [rid])?;
            tx.execute("DELETE FROM docs WHERE rid = ?1", [rid])?;
            changed += 1;
        }
    }

    for doc in docs {
        let hash = doc.hash();
        let current: Option<(i64, String)> = tx
            .query_row(
                "SELECT rid, text_hash FROM docs WHERE id = ?1",
                [&doc.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if current.as_ref().is_some_and(|(_, h)| *h == hash) {
            continue;
        }
        let tags = doc.tags.join(" | ");
        let rid = match current {
            Some((rid, _)) => {
                tx.execute(
                    "UPDATE docs SET name = ?2, tags = ?3, source = ?4, text_hash = ?5 WHERE rid = ?1",
                    params![rid, doc.name, tags, doc.source, hash],
                )?;
                tx.execute("DELETE FROM docs_fts WHERE rowid = ?1", [rid])?;
                rid
            }
            None => {
                tx.execute(
                    "INSERT INTO docs (id, name, tags, source, text_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![doc.id, doc.name, tags, doc.source, hash],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.execute(
            "INSERT INTO docs_fts (rowid, name, description, body, tags, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![rid, doc.name, doc.description, doc.body, tags, doc.source],
        )?;
        changed += 1;
    }
    tx.commit()?;
    Ok(changed)
}

/// One ranked match; `snippet` marks matched body text with
/// [`MARK_START`]/[`MARK_END`] when the match came from SKILL.md.
#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub id: String,
    pub snippet: Option<String>,
}

pub const FILTER_KEYS: &[&str] = &["tag", "source"];

/// Ranked search: name matches beat description matches beat SKILL.md,
/// tags and source; an exact name match always comes first. `tag:` and
/// `source:` narrow by substring.
pub fn search(conn: &Connection, raw: &str) -> rusqlite::Result<Vec<Hit>> {
    let q = parse(raw, FILTER_KEYS);
    let Ok(expr) = match_expr(conn, "docs_vocab", &q.tokens)? else {
        return Ok(Vec::new());
    };
    let mut wheres: Vec<String> = Vec::new();
    let mut args: Vec<Value> = Vec::new();
    for (key, value) in &q.filters {
        let column = if key == "tag" { "d.tags" } else { "d.source" };
        wheres.push(format!("lower({column}) LIKE ?"));
        args.push(Value::Text(format!("%{value}%")));
    }
    let sql = match &expr {
        Some(e) => {
            wheres.insert(0, "docs_fts MATCH ?".into());
            args.insert(0, Value::Text(e.clone()));
            args.push(Value::Text(q.tokens.join(" ")));
            format!(
                "SELECT d.id, snippet(docs_fts, 2, '{MARK_START}', '{MARK_END}', '…', 12)
                 FROM docs_fts JOIN docs d ON d.rid = docs_fts.rowid
                 WHERE {}
                 ORDER BY (lower(d.name) = ?) DESC, bm25(docs_fts, 10.0, 4.0, 1.0, 3.0, 2.0)
                 LIMIT 1000",
                wheres.join(" AND ")
            )
        }
        None => format!(
            "SELECT d.id, NULL FROM docs d {} ORDER BY lower(d.name)",
            if wheres.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", wheres.join(" AND "))
            }
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args), |r| {
        let snippet: Option<String> = r.get(1)?;
        Ok(Hit {
            id: r.get(0)?,
            snippet: snippet.filter(|s| s.contains(MARK_START)),
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::semantic::test_conn;

    fn doc(id: &str, name: &str, description: &str, body: &str, tags: &[&str]) -> Doc {
        Doc {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            body: body.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            source: "git https://github.com/acme/skills".into(),
        }
    }

    fn library() -> Vec<Doc> {
        vec![
            doc("1", "pdf", "Read PDF files", "Extract tables from documents.", &["docs"]),
            doc("2", "xlsx", "Spreadsheet toolkit", "Build Excel spreadsheets with formulas.", &[]),
            doc("3", "git-helper", "Commit messages", "Write good commits.", &["dev"]),
        ]
    }

    fn ids(hits: &[Hit]) -> Vec<String> {
        hits.iter().map(|h| h.id.clone()).collect()
    }

    #[test]
    fn ranks_names_and_tolerates_typos_and_infixes() {
        let mut conn = test_conn();
        sync(&mut conn, &library()).unwrap();
        assert_eq!(ids(&search(&conn, "pdf").unwrap())[0], "1");
        assert_eq!(ids(&search(&conn, "exel").unwrap()), vec!["2"]);
        assert_eq!(ids(&search(&conn, "sheets").unwrap()), vec!["2"]);
        let body = search(&conn, "tables").unwrap();
        assert!(body[0].snippet.as_deref().unwrap().contains(MARK_START));
        assert!(search(&conn, "nonexistentword").unwrap().is_empty());
    }

    #[test]
    fn filters_by_tag_and_source() {
        let mut conn = test_conn();
        sync(&mut conn, &library()).unwrap();
        assert_eq!(ids(&search(&conn, "tag:dev").unwrap()), vec!["3"]);
        assert_eq!(search(&conn, "source:acme").unwrap().len(), 3);
        assert!(search(&conn, "source:elsewhere").unwrap().is_empty());
    }

    #[test]
    fn sync_only_touches_what_changed_and_drops_removed_skills() {
        let mut conn = test_conn();
        assert_eq!(sync(&mut conn, &library()).unwrap(), 3);
        assert_eq!(sync(&mut conn, &library()).unwrap(), 0, "nothing changed");
        let mut edited = library();
        edited[0].description = "Fill PDF forms".into();
        edited.pop();
        assert_eq!(sync(&mut conn, &edited).unwrap(), 2, "one edit, one removal");
        assert_eq!(ids(&search(&conn, "forms").unwrap()), vec!["1"]);
        assert!(search(&conn, "commits").unwrap().is_empty());
    }
}
