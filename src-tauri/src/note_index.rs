//! SQLite index over the vault: notes, links, tags.
//!
//! The database is a rebuildable cache stored inside the vault at
//! `.plinth/index.db`. The markdown files are always the source of truth —
//! the whole index is rebuilt from them every time a vault is opened, so the
//! vault folder can be moved freely.

use crate::link_parser;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct NoteMeta {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SearchHit {
    pub name: String,
    pub snippet: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GraphNode {
    pub name: String,
    /// False for "ghost" nodes: link targets no note file backs yet.
    pub exists: bool,
    pub tags: Vec<String>,
    pub degree: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn open(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    // SQLite's built-in NOCASE folds ASCII only, but Windows resolves file
    // paths case-insensitively for accented letters too (Ü == ü). Left
    // as-is, [[über notes]] would miss the indexed "Über Notes", and the
    // create-on-miss path would overwrite the real file with a stub.
    // Overriding NOCASE with Rust's Unicode lowercasing keeps every
    // COLLATE NOCASE query in agreement with both the filesystem and the
    // graph's canonicalization.
    conn.create_collation("NOCASE", |a, b| a.to_lowercase().cmp(&b.to_lowercase()))?;
    // The index is a rebuildable cache: trade a little durability for a lot
    // of write speed. Losing it in a crash only costs a reindex on reopen.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
             name    TEXT PRIMARY KEY,
             path    TEXT NOT NULL,
             content TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS links (
             source TEXT NOT NULL,
             target TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS tags (
             note TEXT NOT NULL,
             tag  TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS recents (
             note      TEXT PRIMARY KEY,
             opened_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_links_target ON links(target);
         CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);",
    )?;
    Ok(conn)
}

/// What opening a vault found: how many notes were indexed, plus anything
/// the user should know about files that were left alone.
pub struct VaultScan {
    /// Read by tests and available to callers; the command layer only
    /// forwards the warnings.
    #[allow(dead_code)]
    pub notes_indexed: usize,
    pub warnings: Vec<String>,
}

/// Rebuild the whole index from the vault folder. The vault model is flat:
/// only Markdown files in the vault root become notes. Hidden files, editor
/// temp files, and `.plinth` are skipped. Nested Markdown files and
/// case-insensitive duplicate basenames are never indexed silently — they
/// are left untouched on disk and reported as warnings.
pub fn reindex_vault(conn: &Connection, vault: &Path) -> Result<VaultScan, String> {
    // One transaction for the whole rebuild: without it every note is its
    // own fsync'd write and a thousand-note vault takes the better part of
    // a minute to open instead of well under a second.
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| e.to_string())?;
    let result = reindex_vault_inner(conn, vault);
    if result.is_ok() {
        conn.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
    } else {
        let _ = conn.execute_batch("ROLLBACK;");
    }
    result
}

fn reindex_vault_inner(conn: &Connection, vault: &Path) -> Result<VaultScan, String> {
    conn.execute_batch("DELETE FROM notes; DELETE FROM links; DELETE FROM tags;")
        .map_err(|e| e.to_string())?;
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(vault).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if is_root_note_file(&path) {
            candidates.push(path);
        }
    }
    let (unique, mut warnings) = partition_root_files(candidates);

    let mut count = 0;
    for path in unique {
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let content = fs::read_to_string(&path).unwrap_or_default();
        index_note(conn, name, &path.to_string_lossy(), &content).map_err(|e| e.to_string())?;
        count += 1;
    }

    // Nested markdown files are user data Plinth doesn't manage: report
    // them so nobody thinks they were quietly included.
    let mut nested: Vec<String> = Vec::new();
    for entry in WalkDir::new(vault)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        // Root-level files were handled above; only subfolder contents count.
        if entry.depth() < 2 {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("md"))
            == Some(true)
        {
            if let Ok(rel) = path.strip_prefix(vault) {
                nested.push(rel.to_string_lossy().to_string());
            }
        }
    }
    if !nested.is_empty() {
        let sample: Vec<&str> = nested.iter().take(3).map(|s| s.as_str()).collect();
        let suffix = if nested.len() > 3 { ", …" } else { "" };
        warnings.push(format!(
            "{} Markdown file(s) in subfolders were left untouched and not indexed \
             ({}{}). Plinth reads notes from the vault root only — move a file to the \
             vault root to make it a note.",
            nested.len(),
            sample.join(", "),
            suffix
        ));
    }

    Ok(VaultScan {
        notes_indexed: count,
        warnings,
    })
}

/// Is this root-level file one Plinth would index as a note? Markdown only
/// (extension compared case-insensitively), skipping hidden files and
/// editor temp files.
pub(crate) fn is_root_note_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if file_name.starts_with('.') || file_name.starts_with("~$") {
        return false;
    }
    let lower = file_name.to_lowercase();
    lower.ends_with(".md") && lower.len() > 3
}

/// Split candidate note files into indexable ones and warnings about
/// case-insensitive duplicate stems. Duplicates are never resolved by
/// picking a winner: all colliding files are skipped and reported. (On
/// Windows's case-insensitive filesystem such collisions can't normally
/// exist; this protects vaults synced from case-sensitive systems.)
pub(crate) fn partition_root_files(candidates: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<String>) {
    let mut by_stem: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for path in candidates {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        by_stem.entry(stem.to_lowercase()).or_default().push(path);
    }
    let mut unique = Vec::new();
    let mut warnings = Vec::new();
    for (_, files) in by_stem {
        if files.len() > 1 {
            let names: Vec<String> = files
                .iter()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            warnings.push(format!(
                "{} share one note name (names match ignoring case). None of them were \
                 indexed — rename them outside Plinth so each note has a distinct name.",
                names.join(" and ")
            ));
            continue;
        }
        unique.extend(files);
    }
    (unique, warnings)
}

pub(crate) fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
}

/// Upsert a single note and replace its outgoing links and tags.
pub fn index_note(
    conn: &Connection,
    name: &str,
    path: &str,
    content: &str,
) -> rusqlite::Result<()> {
    // Cached prepared statements: this runs once per note in a full
    // reindex, and re-preparing the SQL each call adds up on large vaults.
    conn.prepare_cached(
        "INSERT INTO notes(name, path, content) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET path = ?2, content = ?3",
    )?
    .execute(params![name, path, content])?;
    conn.prepare_cached("DELETE FROM links WHERE source = ?1")?
        .execute(params![name])?;
    conn.prepare_cached("DELETE FROM tags WHERE note = ?1")?
        .execute(params![name])?;
    for target in link_parser::extract_links(content) {
        conn.prepare_cached("INSERT INTO links(source, target) VALUES (?1, ?2)")?
            .execute(params![name, target])?;
    }
    for tag in link_parser::extract_tags(content) {
        conn.prepare_cached("INSERT INTO tags(note, tag) VALUES (?1, ?2)")?
            .execute(params![name, tag])?;
    }
    Ok(())
}

pub fn list_notes(conn: &Connection) -> rusqlite::Result<Vec<NoteMeta>> {
    let mut stmt = conn.prepare("SELECT name, path FROM notes ORDER BY name COLLATE NOCASE")?;
    let rows = stmt.query_map([], |r| {
        Ok(NoteMeta {
            name: r.get(0)?,
            path: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// Resolve a note name (case-insensitively) to its file path and the
/// canonical name stored in the index. Callers must index/track under the
/// canonical name so "[[plinth roadmap]]" and "Plinth Roadmap.md" stay one note.
pub fn note_path(conn: &Connection, name: &str) -> rusqlite::Result<Option<(PathBuf, String)>> {
    let mut stmt = conn.prepare("SELECT path, name FROM notes WHERE name = ?1 COLLATE NOCASE")?;
    let mut rows = stmt.query_map(params![name], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    match rows.next() {
        Some(row) => {
            let (path, canonical) = row?;
            Ok(Some((PathBuf::from(path), canonical)))
        }
        None => Ok(None),
    }
}

/// True when every char of `needle` appears in `hay` in order ("plnth" ~ "plinth").
fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut hay_chars = hay.chars();
    'outer: for nc in needle.chars() {
        for hc in hay_chars.by_ref() {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// Ranked search: exact title substring beats all-terms-in-title beats a
/// fuzzy (subsequence) title match; matching the body adds on top. Every
/// hit carries a context snippet around the first body match.
pub fn search(conn: &Connection, query: &str) -> rusqlite::Result<Vec<SearchHit>> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let terms: Vec<&str> = q.split_whitespace().collect();
    let compact: String = q.chars().filter(|c| !c.is_whitespace()).collect();

    let mut stmt = conn.prepare("SELECT name, content FROM notes")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;

    let mut scored: Vec<(i64, SearchHit)> = Vec::new();
    for row in rows {
        let (name, content) = row?;
        let name_l = name.to_lowercase();
        let content_l = content.to_lowercase();

        let mut score = if name_l.contains(&q) {
            100
        } else if terms.iter().all(|t| name_l.contains(t)) {
            80
        } else if compact.len() >= 3 && is_subsequence(&compact, &name_l) {
            55
        } else {
            0
        };

        // First term that appears in the body anchors the snippet.
        let body_anchor = terms
            .iter()
            .filter_map(|t| content_l.find(t).map(|pos| (pos, *t)))
            .min();
        if terms.iter().all(|t| content_l.contains(t)) {
            score += 35;
        }

        if score > 0 {
            let snippet = match body_anchor {
                Some((_, term)) => make_snippet(&content, term),
                None => content.chars().take(80).collect(),
            };
            scored.push((score, SearchHit { name, snippet }));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    Ok(scored.into_iter().take(50).map(|(_, hit)| hit).collect())
}

/// Record that a note was opened just now (for the Recent section).
pub fn touch_recent(conn: &Connection, name: &str) -> rusqlite::Result<()> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO recents(note, opened_at) VALUES (?1, ?2)
         ON CONFLICT(note) DO UPDATE SET opened_at = ?2",
        params![name, now_ms],
    )?;
    Ok(())
}

/// Most recently opened notes, newest first. The join drops entries whose
/// note has since been deleted or renamed.
pub fn recent_notes(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT r.note FROM recents r JOIN notes n ON n.name = r.note
         ORDER BY r.opened_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| r.get(0))?;
    rows.collect()
}

/// Drop a note from every index table. Backlinks pointing at it stay in
/// other notes' text and simply become create-on-click links again.
pub fn remove_note(conn: &Connection, name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM notes WHERE name = ?1 COLLATE NOCASE",
        params![name],
    )?;
    conn.execute(
        "DELETE FROM links WHERE source = ?1 COLLATE NOCASE",
        params![name],
    )?;
    conn.execute(
        "DELETE FROM tags WHERE note = ?1 COLLATE NOCASE",
        params![name],
    )?;
    conn.execute(
        "DELETE FROM recents WHERE note = ?1 COLLATE NOCASE",
        params![name],
    )?;
    Ok(())
}

fn make_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    let q = query.to_lowercase();
    let Some(pos) = lower.find(&q) else {
        return content.chars().take(80).collect();
    };
    // to_lowercase can shift byte offsets for exotic characters, so clamp to
    // the nearest char boundary in the original text.
    let mut start = pos.saturating_sub(40).min(content.len());
    while start > 0 && !content.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (pos + q.len() + 40).min(content.len());
    while end < content.len() && !content.is_char_boundary(end) {
        end += 1;
    }
    let mut snippet = content[start..end].replace('\n', " ");
    if start > 0 {
        snippet = format!("…{snippet}");
    }
    if end < content.len() {
        snippet.push('…');
    }
    snippet
}

/// Notes whose bodies contain `[[name]]`, case-insensitive on the target.
pub fn backlinks(conn: &Connection, name: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT source FROM links
         WHERE target = ?1 COLLATE NOCASE
         ORDER BY source COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![name], |r| r.get(0))?;
    rows.collect()
}

pub fn all_tags(conn: &Connection) -> rusqlite::Result<Vec<TagCount>> {
    let mut stmt = conn.prepare("SELECT tag, COUNT(*) FROM tags GROUP BY tag ORDER BY tag")?;
    let rows = stmt.query_map([], |r| {
        Ok(TagCount {
            tag: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// The whole vault as a graph: every note a node, every distinct [[link]]
/// an edge. Targets resolve case-insensitively to their canonical note;
/// targets with no note behind them become ghost nodes (exists = false) so
/// broken links show up in the Firmament instead of vanishing.
pub fn graph(conn: &Connection) -> rusqlite::Result<GraphData> {
    let notes = list_notes(conn)?;
    let mut canonical: HashMap<String, String> = HashMap::new();
    for n in &notes {
        canonical.insert(n.name.to_lowercase(), n.name.clone());
    }

    let mut note_tags: HashMap<String, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT note, tag FROM tags ORDER BY tag")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (note, tag) = row?;
        note_tags.entry(note).or_default().push(tag);
    }

    let mut stmt = conn.prepare("SELECT source, target FROM links")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;

    // BTreeMap keeps ghost ordering deterministic across reloads.
    let mut ghosts: BTreeMap<String, String> = BTreeMap::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    for row in rows {
        let (source, raw_target) = row?;
        let target_lower = raw_target.to_lowercase();
        let target = match canonical.get(&target_lower) {
            Some(name) => name.clone(),
            None => ghosts
                .entry(target_lower.clone())
                .or_insert(raw_target)
                .clone(),
        };
        let source_lower = source.to_lowercase();
        // A note linking to itself adds nothing worth drawing.
        if source_lower == target_lower {
            continue;
        }
        if seen.insert((source_lower, target_lower)) {
            edges.push(GraphEdge { source, target });
        }
    }

    let mut degree: HashMap<String, i64> = HashMap::new();
    for e in &edges {
        *degree.entry(e.source.to_lowercase()).or_insert(0) += 1;
        *degree.entry(e.target.to_lowercase()).or_insert(0) += 1;
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    for n in notes {
        nodes.push(GraphNode {
            degree: *degree.get(&n.name.to_lowercase()).unwrap_or(&0),
            tags: note_tags.remove(&n.name).unwrap_or_default(),
            name: n.name,
            exists: true,
        });
    }
    for (lower, display) in ghosts {
        nodes.push(GraphNode {
            name: display,
            exists: false,
            tags: Vec::new(),
            degree: *degree.get(&lower).unwrap_or(&0),
        });
    }
    Ok(GraphData { nodes, edges })
}

pub fn notes_for_tag(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT note FROM tags WHERE tag = ?1 ORDER BY note COLLATE NOCASE")?;
    let rows = stmt.query_map(params![tag.to_lowercase()], |r| r.get(0))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first-milestone flow, minus the GUI: build a vault on disk,
    /// index it, follow links, check backlinks/tags/search.
    #[test]
    fn milestone_flow() {
        let dir = std::env::temp_dir().join(format!("plinth-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("2026-07-03.md"),
            "# 2026-07-03\n\nWorking on [[plinth roadmap]] today, then [[Someday]]. #dev\n",
        )
        .unwrap();
        fs::write(
            dir.join("Plinth Roadmap.md"),
            "# Plinth Roadmap\n\nShip milestone one. #dev #roadmap\n",
        )
        .unwrap();

        let conn = open(&dir.join("index.db")).unwrap();

        let scan = reindex_vault(&conn, &dir).unwrap();
        assert_eq!(scan.notes_indexed, 2);
        assert!(scan.warnings.is_empty());

        // Sidebar list, sorted case-insensitively.
        let names: Vec<String> = list_notes(&conn)
            .unwrap()
            .into_iter()
            .map(|n| n.name)
            .collect();
        assert_eq!(names, vec!["2026-07-03", "Plinth Roadmap"]);

        // [[Plinth Roadmap]] resolves case-insensitively (with spaces) and
        // reports the canonical name; the daily note shows as its backlink.
        let (_, canonical) = note_path(&conn, "plinth roadmap").unwrap().unwrap();
        assert_eq!(canonical, "Plinth Roadmap");
        assert_eq!(
            backlinks(&conn, "Plinth Roadmap").unwrap(),
            vec!["2026-07-03"]
        );

        // Tag explorer: #dev on both notes, #roadmap on one.
        let tags = all_tags(&conn).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!((tags[0].tag.as_str(), tags[0].count), ("dev", 2));
        assert_eq!(
            notes_for_tag(&conn, "roadmap").unwrap(),
            vec!["Plinth Roadmap"]
        );

        // Full-text search hits the body, not just the title.
        let hits = search(&conn, "milestone").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Plinth Roadmap");
        assert!(hits[0].snippet.to_lowercase().contains("milestone"));

        // Fuzzy title match: subsequence with typos-by-omission still finds it.
        let fuzzy = search(&conn, "plnth rdmp").unwrap();
        assert_eq!(fuzzy.len(), 1);
        assert_eq!(fuzzy[0].name, "Plinth Roadmap");

        // Multi-term search must match all terms.
        assert_eq!(search(&conn, "ship nothing").unwrap().len(), 0);
        assert_eq!(search(&conn, "ship milestone").unwrap().len(), 1);

        // The Firmament graph: 2 real notes plus a ghost for the broken
        // link, with the lowercase [[plinth roadmap]] edge resolved to
        // canonical.
        let g = graph(&conn).unwrap();
        assert_eq!(g.nodes.len(), 3);
        let hub = g.nodes.iter().find(|n| n.name == "Plinth Roadmap").unwrap();
        assert!(hub.exists);
        assert_eq!(hub.degree, 1);
        assert_eq!(hub.tags, vec!["dev".to_string(), "roadmap".to_string()]);
        let ghost = g.nodes.iter().find(|n| n.name == "Someday").unwrap();
        assert!(!ghost.exists);
        assert_eq!(ghost.degree, 1);
        assert_eq!(g.edges.len(), 2);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "2026-07-03" && e.target == "Plinth Roadmap"));

        // Recents: most recently opened first, capped, and pruned on delete.
        touch_recent(&conn, "Plinth Roadmap").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        touch_recent(&conn, "2026-07-03").unwrap();
        assert_eq!(
            recent_notes(&conn, 10).unwrap(),
            vec!["2026-07-03", "Plinth Roadmap"]
        );

        remove_note(&conn, "plinth roadmap").unwrap();
        assert_eq!(list_notes(&conn).unwrap().len(), 1);
        assert_eq!(recent_notes(&conn, 10).unwrap(), vec!["2026-07-03"]);
        assert!(all_tags(&conn).unwrap().iter().all(|t| t.tag != "roadmap"));

        fs::remove_dir_all(&dir).ok();
    }

    /// The flat vault model: nested markdown files, hidden files, temp
    /// files and non-markdown files are never indexed — and never touched.
    #[test]
    fn flat_vault_skips_nested_and_irrelevant_files() {
        let dir = std::env::temp_dir().join(format!("plinth-flat-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(dir.join("sub/deeper")).unwrap();
        fs::create_dir_all(dir.join(".plinth")).unwrap();
        fs::create_dir_all(dir.join(".hidden")).unwrap();
        fs::write(dir.join("Root Note.md"), "# Root Note\n\n#real\n").unwrap();
        fs::write(
            dir.join("Second.MD"),
            "# Second\n\ncase-insensitive extension\n",
        )
        .unwrap();
        fs::write(dir.join("sub/Nested.md"), "# Nested\n\nleft alone\n").unwrap();
        fs::write(dir.join("sub/deeper/Deep.md"), "# Deep\n").unwrap();
        fs::write(dir.join(".hidden/Secret.md"), "# Secret\n").unwrap();
        fs::write(dir.join(".plinth/Fake.md"), "# Fake\n").unwrap();
        fs::write(dir.join("~$Temp.md"), "editor temp file").unwrap();
        fs::write(dir.join("notes.txt"), "not markdown").unwrap();
        fs::write(dir.join("export.zip"), "zip bytes").unwrap();

        let conn = open(&dir.join(".plinth/index.db")).unwrap();
        let scan = reindex_vault(&conn, &dir).unwrap();

        // Only the two root-level markdown files became notes.
        assert_eq!(scan.notes_indexed, 2);
        let names: Vec<String> = list_notes(&conn)
            .unwrap()
            .into_iter()
            .map(|n| n.name)
            .collect();
        assert_eq!(names, vec!["Root Note", "Second"]);

        // The nested files are reported (but .plinth/.hidden contents are not).
        assert_eq!(scan.warnings.len(), 1);
        assert!(scan.warnings[0].contains("2 Markdown file(s) in subfolders"));
        assert!(scan.warnings[0].contains("Nested.md"));

        // Nothing on disk was moved, deleted, or rewritten.
        assert_eq!(
            fs::read_to_string(dir.join("sub/Nested.md")).unwrap(),
            "# Nested\n\nleft alone\n"
        );
        assert!(dir.join("sub/deeper/Deep.md").exists());
        assert!(dir.join(".hidden/Secret.md").exists());
        assert!(dir.join("~$Temp.md").exists());

        fs::remove_dir_all(&dir).ok();
    }

    /// Duplicate basenames (possible on case-sensitive filesystems) are
    /// skipped entirely and reported — never resolved by silently picking
    /// one of the files.
    #[test]
    fn duplicate_stems_are_skipped_and_reported() {
        let (unique, warnings) = partition_root_files(vec![
            PathBuf::from("/v/Note.md"),
            PathBuf::from("/v/NOTE.md"),
            PathBuf::from("/v/Other.md"),
        ]);
        assert_eq!(unique, vec![PathBuf::from("/v/Other.md")]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Note.md"));
        assert!(warnings[0].contains("NOTE.md"));
        assert!(warnings[0].contains("None of them were"));
    }

    // ---- performance harness (run with: cargo test perf_ -- --ignored --nocapture)

    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    /// Generate a realistic flat vault: wiki links (some unresolved), tags,
    /// daily-note names, a mix of connected and isolated notes — not empty
    /// placeholder files.
    fn generate_vault(dir: &Path, n: usize) -> Vec<String> {
        const WORDS: [&str; 16] = [
            "signal",
            "durable",
            "layer",
            "market",
            "notebook",
            "argument",
            "practice",
            "milestone",
            "index",
            "vault",
            "foundation",
            "draft",
            "course",
            "evidence",
            "structure",
            "review",
        ];
        const TAGS: [&str; 8] = [
            "ideas", "project", "daily", "reading", "plinth", "research", "draft", "done",
        ];
        fs::create_dir_all(dir).unwrap();
        let mut names: Vec<String> = Vec::with_capacity(n);
        for i in 0..n {
            if i % 7 == 0 {
                // Bijective serial -> date so daily names never collide.
                let d = 1 + i % 28;
                let m = 1 + (i / 28) % 12;
                let y = 2020 + i / (28 * 12);
                names.push(format!("{y:04}-{m:02}-{d:02}"));
            } else {
                names.push(format!("Note {i} {}", WORDS[(i * 31) % WORDS.len()]));
            }
        }
        let mut seed = 42u64;
        for (i, name) in names.iter().enumerate() {
            let mut body = format!("# {name}\n\n");
            let paragraphs = 2 + (lcg(&mut seed) % 3) as usize;
            for _ in 0..paragraphs {
                let words = 20 + (lcg(&mut seed) % 40) as usize;
                for _ in 0..words {
                    body.push_str(WORDS[(lcg(&mut seed) as usize) % WORDS.len()]);
                    body.push(' ');
                }
                body.push_str("\n\n");
            }
            // ~80% of notes link out; the rest stay isolated.
            if lcg(&mut seed) % 10 < 8 {
                let links = 1 + (lcg(&mut seed) % 5) as usize;
                for _ in 0..links {
                    if lcg(&mut seed) % 8 == 0 {
                        body.push_str(&format!(
                            "See [[Unwritten idea {}]].\n",
                            lcg(&mut seed) % 200
                        ));
                    } else {
                        let target = &names[(lcg(&mut seed) as usize) % names.len()];
                        body.push_str(&format!("Related: [[{target}]].\n"));
                    }
                }
            }
            let tags = (lcg(&mut seed) % 4) as usize;
            if tags > 0 {
                body.push('\n');
                for t in 0..tags {
                    body.push_str(&format!("#{} ", TAGS[(i + t * 3) % TAGS.len()]));
                }
                body.push('\n');
            }
            fs::write(dir.join(format!("{name}.md")), body).unwrap();
        }
        names
    }

    fn perf_run(n: usize) {
        use std::time::Instant;
        let dir = std::env::temp_dir().join(format!("plinth-perf-{n}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        let names = generate_vault(&dir, n);
        fs::create_dir_all(dir.join(".plinth")).unwrap();
        let conn = open(&dir.join(".plinth/index.db")).unwrap();

        let t = Instant::now();
        let scan = reindex_vault(&conn, &dir).unwrap();
        let index_ms = t.elapsed().as_millis();
        assert_eq!(scan.notes_indexed, n);

        let t = Instant::now();
        let hits = search(&conn, "durable foundation").unwrap();
        let search_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let title_hits = search(&conn, "note 500").unwrap();
        let title_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let g = graph(&conn).unwrap();
        let graph_ms = t.elapsed().as_millis();

        // A representative watcher refresh: one external edit synced.
        let victim = &names[n / 2];
        fs::write(
            dir.join(format!("{victim}.md")),
            "# changed\n\nExternal edit. [[Note 1 durable]] #changed\n",
        )
        .unwrap();
        let se = std::sync::Mutex::new(crate::watcher::SelfEvents::default());
        let mut paths = std::collections::BTreeSet::new();
        paths.insert(dir.join(format!("{victim}.md")));
        let t = Instant::now();
        let changes = crate::watcher::sync_paths(&conn, &dir, &paths, &se);
        let sync_ms = t.elapsed().as_millis();
        assert_eq!(changes.modified.len(), 1);

        println!(
            "PERF n={n}: index {index_ms} ms | body search {search_ms} ms ({} hits) | \
             title search {title_ms} ms ({} hits) | graph {graph_ms} ms ({} nodes, {} edges) | \
             single-file watcher sync {sync_ms} ms",
            hits.len(),
            title_hits.len(),
            g.nodes.len(),
            g.edges.len()
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore]
    fn perf_1000_notes() {
        perf_run(1000);
    }

    #[test]
    #[ignore]
    fn perf_5000_notes() {
        perf_run(5000);
    }

    #[test]
    fn root_note_file_filter() {
        assert!(is_root_note_file(Path::new("/v/Note.md")));
        assert!(is_root_note_file(Path::new("/v/Note.MD")));
        assert!(!is_root_note_file(Path::new("/v/.hidden.md")));
        assert!(!is_root_note_file(Path::new("/v/~$Note.md")));
        assert!(!is_root_note_file(Path::new("/v/notes.txt")));
        assert!(!is_root_note_file(Path::new("/v/export.zip")));
        assert!(!is_root_note_file(Path::new("/v/.md")));
    }
}
