//! Skill categories. Each category's *description* is embedded and every
//! skill lands in the category whose vector is closest to its own — or
//! in the fallback "Other" when nothing is close enough. Built-ins ship
//! with descriptions written for embedding (concrete tasks, tools and
//! vocabulary, not slogans); user-made categories must supply one.

use super::embed::{dot, from_blob, to_blob};
use super::text_hash;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use serde::Serialize;

/// The fallback category: never embedded, never deletable.
pub const OTHER: &str = "other";

/// Minimum cosine similarity to join a category instead of "Other".
/// MiniLM puts unrelated short texts around 0.0–0.15 and on-topic ones
/// well above 0.3, so this only catches skills that fit nowhere.
pub const MIN_SIMILARITY: f32 = 0.2;

/// Descriptions shorter than this embed as noise.
pub const MIN_DESCRIPTION_CHARS: usize = 20;

struct Builtin {
    id: &'static str,
    emoji: &'static str,
    name: &'static str,
    description: &'static str,
}

const BUILTINS: &[Builtin] = &[
    Builtin {
        id: "development",
        emoji: "💻",
        name: "Development & Code Tools",
        description: "Writing, reviewing, refactoring and debugging source code. Programming languages and frameworks, git workflows, pull requests, code review, linting, testing code, build tools, CI/CD pipelines, APIs, databases, frontend and backend development, deployment and developer tooling.",
    },
    Builtin {
        id: "marketing",
        emoji: "📣",
        name: "Marketing & SEO",
        description: "Marketing campaigns, search engine optimization, keywords and rankings, ad copy, landing pages, social media posts, email newsletters, brand voice, growth, conversion rates, product launches, competitor analysis and audience targeting.",
    },
    Builtin {
        id: "writing",
        emoji: "📝",
        name: "Writing & Research",
        description: "Writing and editing prose: articles, essays, blog posts, reports, summaries and documentation. Grammar, style and tone, outlining, storytelling, citations, literature reviews, fact-checking and researching a topic from sources.",
    },
    Builtin {
        id: "collaboration",
        emoji: "🤝",
        name: "Collaboration & Project Management",
        description: "Planning and coordinating work with a team: project plans, roadmaps, tasks and tickets, issue trackers, sprints, standups, meeting notes, status updates, Slack and email communication, stakeholder alignment, decisions and handoffs.",
    },
    Builtin {
        id: "automation",
        emoji: "⚙️",
        name: "Utility & Automation",
        description: "Automating repetitive tasks and everyday utilities: shell scripts, command-line tools, file management and conversion, scheduling, workflows, browser automation, system setup and configuration, productivity helpers and integrations between apps.",
    },
    Builtin {
        id: "security",
        emoji: "🔐",
        name: "Security & Testing",
        description: "Software security and quality assurance: vulnerability scanning, threat modeling, secrets and authentication, penetration testing, security reviews, unit, integration and end-to-end tests, test coverage, fuzzing and verifying that software behaves correctly.",
    },
    Builtin {
        id: "learning",
        emoji: "📚",
        name: "Learning & Knowledge",
        description: "Learning and teaching: explaining concepts, tutorials, courses, study plans, flashcards, quizzes, note-taking, personal knowledge bases, mentoring, onboarding guides and answering questions about how something works.",
    },
    Builtin {
        id: "creative",
        emoji: "🎨",
        name: "Creative & Design",
        description: "Visual and creative design work: UI and UX design, graphic design, logos, color palettes, typography, illustrations, generative art, design systems, mockups, prototypes, branding visuals and frontend aesthetics.",
    },
    Builtin {
        id: "career",
        emoji: "💼",
        name: "Career & Job Search",
        description: "Careers and job hunting: resumes and CVs, cover letters, LinkedIn profiles, job applications, interview preparation, salary negotiation, career planning and professional development.",
    },
    Builtin {
        id: "media",
        emoji: "🎥",
        name: "Media & Content",
        description: "Audio, video and image content: video editing, podcasts, transcription, subtitles, thumbnails, image generation and editing, GIFs, music, YouTube and streaming content creation.",
    },
    Builtin {
        id: "science",
        emoji: "🔬",
        name: "Scientific & Research Tools",
        description: "Scientific computing and academic research: experiments, lab protocols, bioinformatics, chemistry, physics, mathematics, statistics for papers, scientific literature, academic writing, peer review and reproducible research workflows.",
    },
    Builtin {
        id: "data",
        emoji: "📊",
        name: "Data & Analysis",
        description: "Working with data: analysis, SQL queries, spreadsheets, pandas, data cleaning, visualization, charts and dashboards, metrics, business intelligence, machine learning models and drawing insights from datasets.",
    },
    Builtin {
        id: "documents",
        emoji: "📄",
        name: "Document Skills",
        description: "Reading, creating and editing document files: PDF, Word docx, Excel xlsx, PowerPoint pptx, forms, templates, extracting text and tables from documents, converting between formats and filling in fields.",
    },
];

/// Insert the built-ins and "Other" once; later edits are the user's.
pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("SELECT count(*) FROM categories", [], |r| r.get(0))?;
    if count > 0 {
        return Ok(());
    }
    for (i, b) in BUILTINS.iter().enumerate() {
        conn.execute(
            "INSERT INTO categories (id, name, emoji, description, builtin, position)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![b.id, b.name, b.emoji, b.description, i as i64],
        )?;
    }
    conn.execute(
        "INSERT INTO categories (id, name, emoji, description, builtin, position)
         VALUES (?1, 'Other', '🗂️', 'Skills that fit no other category.', 1, ?2)",
        params![OTHER, BUILTINS.len() as i64],
    )?;
    Ok(())
}

/// The text embedded for a category.
pub fn category_text(name: &str, description: &str) -> String {
    format!("{name}. {description}")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInfo {
    pub id: String,
    pub name: String,
    pub emoji: String,
    pub description: String,
    pub builtin: bool,
    /// The catch-all: can be renamed, never deleted or embedded.
    pub fallback: bool,
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<CategoryInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, description, builtin FROM categories ORDER BY position, name",
    )?;
    let rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        Ok(CategoryInfo {
            fallback: id == OTHER,
            id,
            name: r.get(1)?,
            emoji: r.get(2)?,
            description: r.get(3)?,
            builtin: r.get(4)?,
        })
    })?;
    rows.collect()
}

fn validate(name: &str, description: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("a category needs a name".into());
    }
    if description.trim().chars().count() < MIN_DESCRIPTION_CHARS {
        return Err(format!(
            "describe the category in at least {MIN_DESCRIPTION_CHARS} characters — the description is what skills are matched against"
        ));
    }
    Ok(())
}

/// Create (`id: None`) or update a category. Changing the text clears
/// its vector so the worker re-embeds it and re-sorts every skill.
pub fn save(
    conn: &Connection,
    id: Option<&str>,
    name: &str,
    emoji: &str,
    description: &str,
) -> Result<CategoryInfo, String> {
    let (name, emoji, description) = (name.trim(), emoji.trim(), description.trim());
    let fallback = id == Some(OTHER);
    if fallback {
        if name.is_empty() {
            return Err("a category needs a name".into());
        }
    } else {
        validate(name, description)?;
    }
    let e = |e: rusqlite::Error| e.to_string();
    let id = match id {
        Some(id) => {
            let changed = conn
                .execute(
                    "UPDATE categories SET name = ?2, emoji = ?3, description = ?4,
                         vector = NULL
                     WHERE id = ?1",
                    params![id, name, emoji, description],
                )
                .map_err(e)?;
            if changed == 0 {
                return Err(format!("no category '{id}'"));
            }
            id.to_string()
        }
        None => {
            let id = format!("user-{}", uuid::Uuid::new_v4());
            // new categories go just before "Other"
            let position: i64 = conn
                .query_row(
                    "SELECT coalesce(max(position), 0) FROM categories WHERE id != ?1",
                    [OTHER],
                    |r| r.get(0),
                )
                .map_err(e)?;
            conn.execute(
                "INSERT INTO categories (id, name, emoji, description, builtin, position)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)",
                params![id, name, emoji, description, position + 1],
            )
            .map_err(e)?;
            conn.execute(
                "UPDATE categories SET position = ?2 WHERE id = ?1",
                params![OTHER, position + 2],
            )
            .map_err(e)?;
            id
        }
    };
    list(conn)
        .map_err(e)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "category vanished after saving".into())
}

/// Delete a category; its skills are re-sorted into the rest.
pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    if id == OTHER {
        return Err("'Other' is where unmatched skills go and can't be deleted".into());
    }
    let e = |e: rusqlite::Error| e.to_string();
    conn.execute("DELETE FROM categories WHERE id = ?1", [id])
        .map_err(e)?;
    assign_all(conn).map_err(e)
}

/// Categories still needing a vector: `(id, text)`.
pub fn unembedded(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description FROM categories WHERE vector IS NULL AND id != ?1",
    )?;
    let rows = stmt.query_map([OTHER], |r| {
        let name: String = r.get(1)?;
        let description: String = r.get(2)?;
        Ok((r.get(0)?, category_text(&name, &description)))
    })?;
    rows.collect()
}

pub fn set_vector(conn: &Connection, id: &str, vector: &[u8]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE categories SET vector = ?2 WHERE id = ?1",
        params![id, vector],
    )?;
    Ok(())
}

fn category_vectors(conn: &Connection) -> rusqlite::Result<Vec<(String, Vec<f32>)>> {
    let mut stmt =
        conn.prepare("SELECT id, vector FROM categories WHERE vector IS NOT NULL AND id != ?1")?;
    let rows = stmt.query_map([OTHER], |r| {
        let blob: Vec<u8> = r.get(1)?;
        Ok((r.get::<_, String>(0)?, from_blob(&blob)))
    })?;
    rows.collect()
}

/// Nearest category for a vector, or "Other" when none is close enough.
pub fn nearest<'a>(vector: &[f32], categories: &'a [(String, Vec<f32>)]) -> &'a str {
    nearest_above(vector, categories, MIN_SIMILARITY)
}

/// [`nearest`] with an explicit bar for joining a category.
pub fn nearest_above<'a>(vector: &[f32], categories: &'a [(String, Vec<f32>)], min: f32) -> &'a str {
    categories
        .iter()
        .map(|(id, c)| (id.as_str(), dot(vector, c)))
        .filter(|(_, score)| *score >= min)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(OTHER, |(id, _)| id)
}

/// Re-sort every embedded skill — after a category is added, edited
/// (and re-embedded) or deleted. A few thousand dot products; cheap.
pub fn assign_all(conn: &Connection) -> rusqlite::Result<()> {
    let categories = category_vectors(conn)?;
    let mut stmt = conn.prepare("SELECT id, vector FROM embeddings")?;
    let rows: Vec<(String, Vec<u8>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    let tx = conn.unchecked_transaction()?;
    for (id, blob) in rows {
        tx.execute(
            "UPDATE embeddings SET category_id = ?2 WHERE id = ?1",
            params![id, nearest(&from_blob(&blob), &categories)],
        )?;
    }
    tx.commit()
}

/// Store one embedded skill and sort it immediately.
pub fn put_embedding(
    conn: &Connection,
    id: &str,
    hash: &str,
    vector: &[f32],
    categories: &[(String, Vec<f32>)],
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO embeddings (id, text_hash, vector, category_id) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            text_hash = excluded.text_hash, vector = excluded.vector,
            category_id = excluded.category_id",
        params![id, hash, to_blob(vector), nearest(vector, categories)],
    )?;
    Ok(())
}

/// Library skills whose embedded text is new or changed: `(id, text,
/// hash)`. Vectors of skills that left the library are dropped.
pub fn pending(conn: &Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    conn.execute("DELETE FROM embeddings WHERE id NOT IN (SELECT id FROM docs)", [])?;
    let mut stmt = conn.prepare(
        "SELECT d.id, f.name, f.description, f.body, e.text_hash
         FROM docs d JOIN docs_fts f ON f.rowid = d.rid
         LEFT JOIN embeddings e ON e.id = d.id",
    )?;
    let rows: Vec<(String, String, String, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, name, description, body, stored)| {
            let text = skill_text(&name, &description, &body);
            let hash = text_hash(&text);
            (stored.as_deref() != Some(hash.as_str())).then_some((id, text, hash))
        })
        .collect())
}

/// Characters of SKILL.md body used when the description is thin.
const BODY_EXCERPT_CHARS: usize = 600;

pub fn skill_text(name: &str, description: &str, body: &str) -> String {
    let mut text = format!("{}. {}", name.replace(['-', '_'], " "), description.trim());
    if description.trim().chars().count() < 60 && !body.is_empty() {
        let excerpt: String = strip_frontmatter(body)
            .chars()
            .take(BODY_EXCERPT_CHARS)
            .collect();
        text.push(' ');
        text.push_str(excerpt.trim());
    }
    text
}

fn strip_frontmatter(body: &str) -> &str {
    if let Some(rest) = body.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            return &rest[end + 4..];
        }
    }
    body
}

pub fn categories_with_vectors(conn: &Connection) -> rusqlite::Result<Vec<(String, Vec<f32>)>> {
    category_vectors(conn)
}

/// Library skill id → category id, for every embedded skill.
pub fn assignments(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let mut stmt =
        conn.prepare("SELECT id, category_id FROM embeddings WHERE category_id IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}
/// Most items one `categorize` call accepts — a marketplace page, or a
/// repository's skills, with room to spare.
pub const MAX_ITEMS: usize = 5000;

/// Texts this short are a bare name ("pdf-tools"): every similarity runs
/// low, so the bar for joining a category is lowered to match.
const SHORT_TEXT_CHARS: usize = 40;
const SHORT_TEXT_MIN_SIMILARITY: f32 = 0.08;

/// Category per item for skills outside the library. Cached vectors are
/// reused by text; `embed` is only called for texts never seen before.
pub fn categorize(
    conn: &Connection,
    items: &[(String, String)],
    mut embed: impl FnMut(&[String]) -> Result<Vec<Vec<f32>>, String>,
) -> Result<HashMap<String, String>, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let cats = category_vectors(conn).map_err(e)?;
    let hashed: Vec<(&str, &str, String)> = items
        .iter()
        .take(MAX_ITEMS)
        .map(|(key, text)| (key.as_str(), text.as_str(), text_hash(text)))
        .collect();

    let mut vectors: HashMap<String, Vec<f32>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT vector FROM remote_vectors WHERE text_hash = ?1")
            .map_err(e)?;
        for (_, _, hash) in &hashed {
            if vectors.contains_key(hash) {
                continue;
            }
            if let Some(blob) = stmt
                .query_row([hash], |r| r.get::<_, Vec<u8>>(0))
                .optional()
                .map_err(e)?
            {
                vectors.insert(hash.clone(), from_blob(&blob));
            }
        }
    }

    let mut missing: Vec<(String, String)> = Vec::new();
    for (_, text, hash) in &hashed {
        if !vectors.contains_key(hash) && !missing.iter().any(|(h, _)| h == hash) {
            missing.push((hash.clone(), text.to_string()));
        }
    }
    for chunk in missing.chunks(32) {
        let texts: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();
        let embedded = embed(&texts)?;
        let tx = conn.unchecked_transaction().map_err(e)?;
        for ((hash, _), v) in chunk.iter().zip(embedded) {
            tx.execute(
                "INSERT OR REPLACE INTO remote_vectors (text_hash, vector) VALUES (?1, ?2)",
                params![hash, to_blob(&v)],
            )
            .map_err(e)?;
            vectors.insert(hash.clone(), v);
        }
        tx.commit().map_err(e)?;
    }

    Ok(hashed
        .into_iter()
        .filter_map(|(key, text, hash)| {
            let min = if text.chars().count() < SHORT_TEXT_CHARS {
                SHORT_TEXT_MIN_SIMILARITY
            } else {
                MIN_SIMILARITY
            };
            vectors
                .get(&hash)
                .map(|v| (key.to_string(), nearest_above(v, &cats, min).to_string()))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::semantic::index::{sync, Doc};
    use crate::core::semantic::test_conn;

    fn unit(v: &[f32]) -> Vec<f32> {
        let n = dot(v, v).sqrt();
        v.iter().map(|x| x / n).collect()
    }

    #[test]
    fn seed_once_with_every_builtin_and_other() {
        let conn = test_conn();
        let all = list(&conn).unwrap();
        assert_eq!(all.len(), BUILTINS.len() + 1);
        assert!(all.last().unwrap().fallback, "Other sorts last");
        save(&conn, Some("data"), "Data", "📊", "renamed but still long enough").unwrap();
        seed(&conn).unwrap();
        assert_eq!(list(&conn).unwrap().len(), BUILTINS.len() + 1, "no reseed");
    }

    #[test]
    fn user_categories_require_a_description_and_land_before_other() {
        let conn = test_conn();
        assert!(save(&conn, None, "Games", "🎮", "fun").is_err());
        let made = save(&conn, None, "Games", "🎮", "Game development, engines and level design").unwrap();
        let all = list(&conn).unwrap();
        assert_eq!(all[all.len() - 2].id, made.id);
        assert!(delete(&conn, OTHER).is_err());
        delete(&conn, &made.id).unwrap();
    }

    #[test]
    fn nearest_falls_back_to_other_below_threshold() {
        let cats = vec![("a".to_string(), unit(&[1.0, 0.0])), ("b".to_string(), unit(&[0.0, 1.0]))];
        assert_eq!(nearest(&unit(&[0.9, 0.1]), &cats), "a");
        assert_eq!(nearest(&unit(&[-1.0, -1.0]), &cats), OTHER);
        assert_eq!(nearest(&[1.0, 0.0], &[]), OTHER);
    }

    #[test]
    fn pending_tracks_index_changes_and_editing_resorts() {
        let mut conn = test_conn();
        let doc = |id: &str, d: &str| Doc {
            id: id.into(), name: format!("skill-{id}"), description: d.into(),
            body: String::new(), tags: vec![], source: String::new(),
        };
        sync(&mut conn, &[doc("1", "first"), doc("2", "second")]).unwrap();
        let todo = pending(&conn).unwrap();
        assert_eq!(todo.len(), 2);

        conn.execute("UPDATE categories SET vector = NULL", []).unwrap();
        set_vector(&conn, "data", &to_blob(&unit(&[1.0, 0.0]))).unwrap();
        set_vector(&conn, "career", &to_blob(&unit(&[0.0, 1.0]))).unwrap();
        let cats = categories_with_vectors(&conn).unwrap();
        for (id, _, hash) in &todo {
            put_embedding(&conn, id, hash, &unit(&[1.0, 0.5]), &cats).unwrap();
        }
        assert!(pending(&conn).unwrap().is_empty(), "nothing left to embed");
        assert_eq!(assignments(&conn).unwrap()["1"], "data");

        save(&conn, Some("data"), "Data", "📊", "a new description for data work").unwrap();
        assign_all(&conn).unwrap();
        assert_eq!(assignments(&conn).unwrap()["1"], "career", "re-sorted without data's vector");

        // a skill leaving the library takes its vector with it
        sync(&mut conn, &[doc("1", "first")]).unwrap();
        pending(&conn).unwrap();
        assert!(!assignments(&conn).unwrap().contains_key("2"));
    }

    #[test]
    fn thin_descriptions_borrow_from_the_body() {
        let t = skill_text("git-helper", "git", "---\nname: x\n---\nWrites commit messages.");
        assert!(t.starts_with("git helper. git"));
        assert!(t.contains("Writes commit messages") && !t.contains("name: x"));
    }

    #[test]
    fn categorize_embeds_each_new_text_once_and_reuses_the_cache() {
        let conn = test_conn();
        conn.execute("UPDATE categories SET vector = NULL", []).unwrap();
        set_vector(&conn, "data", &to_blob(&unit(&[1.0, 0.0]))).unwrap();
        set_vector(&conn, "career", &to_blob(&unit(&[0.0, 1.0]))).unwrap();
        let calls = std::cell::Cell::new(0);
        let mut fake = |texts: &[String]| {
            calls.set(calls.get() + texts.len());
            Ok(texts
                .iter()
                .map(|t| if t.contains("sql") { unit(&[1.0, 0.1]) } else { unit(&[0.1, 1.0]) })
                .collect())
        };
        let items = vec![
            ("a".to_string(), "sql queries".to_string()),
            ("b".to_string(), "resume writer".to_string()),
            ("c".to_string(), "sql queries".to_string()),
        ];
        let got = categorize(&conn, &items, &mut fake).unwrap();
        assert_eq!(got["a"], "data");
        assert_eq!(got["b"], "career");
        assert_eq!(got["c"], "data");
        assert_eq!(calls.get(), 2, "identical texts embedded once");
        categorize(&conn, &items, &mut fake).unwrap();
        assert_eq!(calls.get(), 2, "second call is served from the cache");
    }
}

/// Manual check of category quality on the bundled catalog:
/// `cargo test --release category_eval -- --ignored --nocapture`
#[cfg(test)]
mod eval {
    use super::*;
    use crate::core::semantic::embed::Embedder;
    use crate::core::semantic::test_conn;

    #[test]
    #[ignore]
    fn category_eval() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/minilm");
        let e = Embedder::load(&dir).unwrap();
        let conn = test_conn();
        let cats = unembedded(&conn).unwrap();
        let texts: Vec<String> = cats.iter().map(|(_, t)| t.clone()).collect();
        for ((id, _), v) in cats.iter().zip(e.embed(&texts).unwrap()) {
            set_vector(&conn, id, &to_blob(&v)).unwrap();
        }
        let cv = categories_with_vectors(&conn).unwrap();
        let raw: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/data/skill-catalog.json")).unwrap(),
        )
        .unwrap();
        let sample: Vec<(String, String)> = raw.as_array().unwrap().iter().step_by(7).take(400)
            .map(|s| (s["name"].as_str().unwrap().replace(['-', '_'], " "), s["description"].as_str().unwrap_or("").to_string()))
            .collect();
        let names: Vec<String> = sample.iter().map(|(n, _)| n.clone()).collect();
        let full: Vec<String> = sample.iter().map(|(n, d)| format!("{n}. {d}")).collect();
        let nv: Vec<Vec<f32>> = names.chunks(32).flat_map(|c| e.embed(c).unwrap()).collect();
        let fv: Vec<Vec<f32>> = full.chunks(32).flat_map(|c| e.embed(c).unwrap()).collect();
        let other = |vs: &[Vec<f32>], min: f32| vs.iter().filter(|v| nearest_above(v, &cv, min) == OTHER).count();
        for min in [0.2, 0.15, 0.1, 0.08, 0.05] {
            println!("names only, min {min}: {} / {} in Other", other(&nv, min), nv.len());
        }
        println!("name + description, min 0.2: {} / {} in Other", other(&fv, 0.2), fv.len());
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for v in &fv { *by.entry(nearest(v, &cv).to_string()).or_default() += 1; }
        println!("name + description split: {by:?}");
        for (i, v) in nv.iter().enumerate().take(12) {
            println!("  {:<34} names→{:<14} full→{}", names[i], nearest_above(v, &cv, 0.08), nearest(&fv[i], &cv));
        }
    }
}
