//! Query language + FTS5 query building shared by the installed-skill
//! index and the remote catalog.
//!
//! A query is free text plus optional `key:value` filters, e.g.
//! `pdf tool:cursor enabled:no`. Free text is tokenized like the index
//! (unicode61), every token must match (AND), and each token matches as
//! a prefix (`spread` → `spreadsheet`), as an infix (`sheet` →
//! `spreadsheet`), and — only when nothing starts with it — within a
//! small edit distance (`exel` → `excel`). Candidate terms come from the
//! index's own vocabulary, so expansion never invents words.

use rusqlite::Connection;

/// Snippet highlight markers. Control characters can't appear in skill
/// text, so the UI can split on them without trusting any markup.
pub const MARK_START: char = '\u{2}';
pub const MARK_END: char = '\u{3}';

#[derive(Debug, Default, PartialEq)]
pub struct ParsedQuery {
    /// Lowercased tokens, each a required match.
    pub tokens: Vec<String>,
    /// Recognised `key:value` filters, key lowercased.
    pub filters: Vec<(String, String)>,
}

/// Split a raw query into tokens and filters. Only keys in `known` are
/// filters; anything else (`c++`, `http://…`) stays searchable text.
pub fn parse(raw: &str, known: &[&str]) -> ParsedQuery {
    let mut out = ParsedQuery::default();
    for word in raw.split_whitespace() {
        if let Some((key, value)) = word.split_once(':') {
            let key = key.to_lowercase();
            if known.contains(&key.as_str()) && !value.is_empty() {
                out.filters.push((key, value.to_lowercase()));
                continue;
            }
        }
        out.tokens.extend(tokenize(word));
    }
    out
}

/// Mirror unicode61: alphanumeric runs, lowercased.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

fn max_edits(token: &str) -> usize {
    match token.chars().count() {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    }
}

/// Levenshtein distance with an early exit once every cell in a row
/// exceeds `limit` — vocabularies are small, but this keeps it linear-ish.
fn edit_distance(a: &str, b: &str, limit: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > limit {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        if cur.iter().all(|&d| d > limit) {
            return None;
        }
        prev = cur;
    }
    (prev[b.len()] <= limit).then_some(prev[b.len()])
}

fn quote(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

/// Most expansion candidates kept per token — enough to catch the
/// intended word, few enough that one typo can't match everything.
const MAX_EXPANSIONS: usize = 8;

/// One token's MATCH alternatives, or `None` when nothing in `vocab`
/// could match it (the whole AND query then cannot match).
fn token_clause(token: &str, vocab: &[String]) -> Option<String> {
    let has_prefix = vocab.iter().any(|t| t.starts_with(token));
    let mut alts = Vec::new();
    if has_prefix {
        alts.push(format!("{}*", quote(token)));
    }
    if token.chars().count() >= 3 {
        alts.extend(
            vocab
                .iter()
                .filter(|t| !t.starts_with(token) && t.contains(token))
                .take(MAX_EXPANSIONS)
                .map(|t| quote(t)),
        );
    }
    if !has_prefix {
        let limit = max_edits(token);
        if limit > 0 {
            let mut near: Vec<(usize, &String)> = vocab
                .iter()
                .filter_map(|t| edit_distance(token, t, limit).map(|d| (d, t)))
                .collect();
            near.sort();
            alts.extend(near.into_iter().take(MAX_EXPANSIONS).map(|(_, t)| quote(t)));
        }
    }
    alts.dedup();
    match alts.len() {
        0 => None,
        1 => alts.pop(),
        _ => Some(format!("({})", alts.join(" OR "))),
    }
}

/// Build an FTS5 MATCH expression for `tokens` against a vocabulary
/// table. `Ok(None)` means "no tokens" (filters only); `Err(())` means
/// some token can't match anything.
pub fn match_expr(
    conn: &Connection,
    vocab_table: &str,
    tokens: &[String],
) -> rusqlite::Result<Result<Option<String>, ()>> {
    if tokens.is_empty() {
        return Ok(Ok(None));
    }
    let mut stmt = conn.prepare(&format!("SELECT term FROM {vocab_table}"))?;
    let vocab: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut clauses = Vec::with_capacity(tokens.len());
    for token in tokens {
        match token_clause(token, &vocab) {
            Some(c) => clauses.push(c),
            None => return Ok(Err(())),
        }
    }
    Ok(Ok(Some(clauses.join(" AND "))))
}

pub fn truthy(value: &str) -> Option<bool> {
    match value {
        "yes" | "true" | "on" | "1" | "enabled" => Some(true),
        "no" | "false" | "off" | "0" | "disabled" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn parse_splits_filters_from_text() {
        let q = parse("PDF tool:Cursor c++ foo:bar", &["tool"]);
        assert_eq!(q.filters, vec![("tool".into(), "cursor".into())]);
        assert_eq!(q.tokens, vec!["pdf", "c", "foo", "bar"]);
    }

    #[test]
    fn prefix_infix_and_typo_expansion() {
        let v = vocab(&["spreadsheet", "spreadsheets", "excel", "pdf", "sheets"]);
        assert_eq!(token_clause("pd", &v).unwrap(), "\"pd\"*");
        let infix = token_clause("sheet", &v).unwrap();
        assert!(infix.contains("\"sheet\"*") && infix.contains("\"spreadsheet\""));
        assert_eq!(token_clause("exel", &v).unwrap(), "\"excel\"");
        assert!(token_clause("zzz", &v).is_none());
    }

    #[test]
    fn short_tokens_get_no_typo_expansion() {
        let v = vocab(&["git", "gif"]);
        assert!(token_clause("gix", &v).is_none());
    }

    #[test]
    fn edit_distance_respects_the_limit() {
        assert_eq!(edit_distance("kitten", "sitting", 3), Some(3));
        assert_eq!(edit_distance("kitten", "sitting", 2), None);
        assert_eq!(edit_distance("abc", "abc", 0), Some(0));
    }

    #[test]
    fn quotes_are_escaped() {
        assert_eq!(quote("a\"b"), "\"a\"\"b\"");
    }
}
