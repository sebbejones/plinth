//! Operations that touch both the vault's files and its index together:
//! name validation, conflict-checked writes, and note renaming.
//!
//! Everything here works on a `Connection` plus a vault root path, so it can
//! be tested without a running Tauri app.

use crate::note_index::{self, NoteMeta};
use crate::watcher::SelfEvents;
use regex::Regex;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Stable FNV-1a hash of a note body, hex-encoded. Used by the editor as a
/// "this is the disk state I last saw" token, and by the watcher to tell
/// Plinth's own saves apart from external edits.
pub fn content_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Reject note names that could escape the vault, collide with Plinth's
/// internals, or be unrepresentable on Windows. Callers pass trimmed names.
pub fn validate_note_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Note name is empty.".into());
    }
    if name.contains("..") {
        return Err("Note names can't contain \"..\".".into());
    }
    if let Some(c) = name.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (*c as u32) < 0x20
    }) {
        return Err(format!(
            "Note names can't contain \"{}\".",
            c.escape_default()
        ));
    }
    if name.starts_with('.') {
        return Err("Note names can't start with a dot.".into());
    }
    if name.ends_with('.') {
        return Err("Note names can't end with a dot.".into());
    }
    if RESERVED_NAMES.contains(&name.to_ascii_uppercase().as_str()) {
        return Err(format!("\"{name}\" is a reserved name on Windows."));
    }
    Ok(())
}

/// Result of a conflict-checked write, as one tagged value the frontend can
/// branch on. `saved` carries the fresh note list and content hash;
/// `conflict` carries the current disk content so the user can choose;
/// `missing` means the file was deleted externally and nothing was written.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WriteResult {
    pub status: String,
    pub notes: Vec<NoteMeta>,
    pub hash: String,
    pub disk: String,
}

impl WriteResult {
    fn saved(notes: Vec<NoteMeta>, hash: String) -> Self {
        WriteResult {
            status: "saved".into(),
            notes,
            hash,
            disk: String::new(),
        }
    }
    fn conflict(disk: String) -> Self {
        WriteResult {
            status: "conflict".into(),
            notes: Vec::new(),
            hash: String::new(),
            disk,
        }
    }
    fn missing() -> Self {
        WriteResult {
            status: "missing".into(),
            notes: Vec::new(),
            hash: String::new(),
            disk: String::new(),
        }
    }
}

/// Write a note, refusing to clobber external edits. `base_hash` is the hash
/// of the disk content the editor last loaded or saved; if the file on disk
/// no longer matches it (and doesn't already equal the new content), nothing
/// is written and the disk version is returned instead. `base_hash = None`
/// writes unconditionally — used only after the user explicitly chooses
/// "keep my version" in a conflict.
pub fn write_note(
    conn: &Connection,
    root: &Path,
    self_events: &Mutex<SelfEvents>,
    name: &str,
    content: &str,
    base_hash: Option<&str>,
) -> Result<WriteResult, String> {
    validate_note_name(name)?;
    let (path, canonical) = match note_index::note_path(conn, name).map_err(|e| e.to_string())? {
        Some(resolved) => resolved,
        None => (root.join(format!("{name}.md")), name.to_string()),
    };
    if let Some(base) = base_hash {
        match fs::read_to_string(&path) {
            Ok(disk) => {
                if content_hash(&disk) != base && disk != content {
                    return Ok(WriteResult::conflict(disk));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(WriteResult::missing());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let hash = content_hash(content);
    if let Ok(mut se) = self_events.lock() {
        se.note_write(&path, &hash);
    }
    fs::write(&path, content).map_err(|e| e.to_string())?;
    note_index::index_note(conn, &canonical, &path.to_string_lossy(), content)
        .map_err(|e| e.to_string())?;
    let notes = note_index::list_notes(conn).map_err(|e| e.to_string())?;
    Ok(WriteResult::saved(notes, hash))
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RenameOutcome {
    pub notes: Vec<NoteMeta>,
    pub name: String,
    pub links_updated: usize,
}

/// Rename a note's file, carry its index rows over, and rewrite every
/// `[[link]]` in the vault that points at the old name. Only wiki-link
/// targets are touched — plain prose containing the old name is left alone.
///
/// If the file rename itself fails, nothing has changed. If a link rewrite
/// fails afterwards, the index is rebuilt from disk (so it is never stale)
/// and the error names the notes that still link to the old name.
pub fn rename_note(
    conn: &Connection,
    root: &Path,
    self_events: &Mutex<SelfEvents>,
    old_name: &str,
    new_name: &str,
) -> Result<RenameOutcome, String> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_note_name(new_name)?;
    let (old_path, canonical_old) = note_index::note_path(conn, old_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Note \"{old_name}\" was not found."))?;
    if canonical_old == new_name {
        return Err("That is already the note's name.".into());
    }
    let case_only = canonical_old.to_lowercase() == new_name.to_lowercase();
    let new_path = root.join(format!("{new_name}.md"));
    if !case_only {
        if note_index::note_path(conn, new_name)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err(format!("A note named \"{new_name}\" already exists."));
        }
        if new_path.exists() {
            return Err(format!(
                "A file named \"{new_name}.md\" already exists in the vault folder."
            ));
        }
    }

    fs::rename(&old_path, &new_path)
        .map_err(|e| format!("Couldn't rename the file: {e}. Nothing was changed."))?;
    let content = fs::read_to_string(&new_path).map_err(|e| e.to_string())?;
    if let Ok(mut se) = self_events.lock() {
        se.note_delete(&old_path);
        se.note_write(&new_path, &content_hash(&content));
    }

    // Carry the note's own rows over to the new name before touching links,
    // so the incoming-link query below sees consistent source names.
    let new_path_str = new_path.to_string_lossy().to_string();
    (|| -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE notes SET name = ?1, path = ?2 WHERE name = ?3",
            params![new_name, new_path_str, canonical_old],
        )?;
        conn.execute(
            "UPDATE links SET source = ?1 WHERE source = ?2",
            params![new_name, canonical_old],
        )?;
        conn.execute(
            "UPDATE tags SET note = ?1 WHERE note = ?2",
            params![new_name, canonical_old],
        )?;
        // A stale recents row under the new name (from a long-deleted note)
        // would collide with the primary key, so clear it first.
        conn.execute(
            "DELETE FROM recents WHERE note = ?1 COLLATE NOCASE AND note != ?2",
            params![new_name, canonical_old],
        )?;
        conn.execute(
            "UPDATE recents SET note = ?1 WHERE note = ?2",
            params![new_name, canonical_old],
        )?;
        Ok(())
    })()
    .map_err(|e| e.to_string())?;

    // Rewrite [[old name]] (any casing, any inner padding) in every note
    // that links to it — including the renamed note itself if it self-links.
    let link_re = Regex::new(&format!(
        r"(?i)\[\[\s*{}\s*\]\]",
        regex::escape(&canonical_old)
    ))
    .map_err(|e| e.to_string())?;
    let sources: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT n.name, n.path FROM links l
                 JOIN notes n ON n.name = l.source
                 WHERE l.target = ?1 COLLATE NOCASE",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![canonical_old], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    let mut links_updated = 0;
    let mut failed: Vec<String> = Vec::new();
    for (src_name, src_path) in sources {
        let result = (|| -> Result<usize, String> {
            let text = fs::read_to_string(&src_path).map_err(|e| e.to_string())?;
            let n = link_re.find_iter(&text).count();
            if n == 0 {
                return Ok(0);
            }
            let new_text =
                link_re.replace_all(&text, |_: &regex::Captures| format!("[[{new_name}]]"));
            if let Ok(mut se) = self_events.lock() {
                se.note_write(Path::new(&src_path), &content_hash(&new_text));
            }
            fs::write(&src_path, new_text.as_bytes()).map_err(|e| e.to_string())?;
            note_index::index_note(conn, &src_name, &src_path, &new_text)
                .map_err(|e| e.to_string())?;
            Ok(n)
        })();
        match result {
            Ok(n) => links_updated += n,
            Err(e) => failed.push(format!("{src_name} ({e})")),
        }
    }

    if !failed.is_empty() {
        // Don't leave the index guessing: rebuild it from what's on disk.
        let _ = note_index::reindex_vault(conn, root);
        return Err(format!(
            "The note is now \"{new_name}\", but its old name is still linked from: {}. \
             Those notes could not be rewritten — fix the [[{canonical_old}]] links by hand.",
            failed.join(", ")
        ));
    }

    let notes = note_index::list_notes(conn).map_err(|e| e.to_string())?;
    Ok(RenameOutcome {
        notes,
        name: new_name.to_string(),
        links_updated,
    })
}

/// True when this path may be used as a vault root. Rejects `.plinth` (and
/// anything inside one) so the index folder can never become a vault and
/// nest another `.plinth` inside itself.
pub fn validate_vault_root(root: &Path) -> Result<(), String> {
    for comp in root.components() {
        if comp
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(".plinth")
        {
            return Err(
                "That folder is Plinth's internal index folder (.plinth), not a vault. \
                 Choose the folder that holds your Markdown notes."
                    .into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_index;
    use std::path::PathBuf;

    fn temp_vault(tag: &str) -> (PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("plinth-ops-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(dir.join(".plinth")).unwrap();
        let conn = note_index::open(&dir.join(".plinth/index.db")).unwrap();
        (dir, conn)
    }

    fn se() -> Mutex<SelfEvents> {
        Mutex::new(SelfEvents::default())
    }

    #[test]
    fn name_validation() {
        assert!(validate_note_name("Plinth Roadmap").is_ok());
        assert!(validate_note_name("2026-07-23").is_ok());
        assert!(validate_note_name("Ünïcode Näme").is_ok());
        assert!(validate_note_name("What now?!").is_err()); // '?'
        assert!(validate_note_name("").is_err());
        assert!(validate_note_name("a/b").is_err());
        assert!(validate_note_name("a\\b").is_err());
        assert!(validate_note_name("C:evil").is_err());
        assert!(validate_note_name("..").is_err());
        assert!(validate_note_name("up..dir").is_err());
        assert!(validate_note_name("a*b").is_err());
        assert!(validate_note_name("a\"b").is_err());
        assert!(validate_note_name("a<b>c").is_err());
        assert!(validate_note_name("a|b").is_err());
        assert!(validate_note_name(".hidden").is_err());
        assert!(validate_note_name(".plinth").is_err());
        assert!(validate_note_name("trailing.").is_err());
        assert!(validate_note_name("con").is_err());
        assert!(validate_note_name("COM3").is_err());
        assert!(validate_note_name("lpt9").is_err());
        assert!(validate_note_name("Nul").is_err());
    }

    #[test]
    fn vault_root_validation() {
        assert!(validate_vault_root(Path::new("C:/Users/x/Notes")).is_ok());
        assert!(validate_vault_root(Path::new("C:/Users/x/Notes/.plinth")).is_err());
        assert!(validate_vault_root(Path::new("C:/Users/x/.PLINTH/sub")).is_err());
    }

    #[test]
    fn hash_is_stable_and_distinguishes() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
        assert_ne!(content_hash(""), content_hash(" "));
    }

    /// The autosave path: a matching base hash writes; a disk file that
    /// moved on without us returns the disk content instead of clobbering
    /// it; a vanished file reports missing instead of recreating itself.
    #[test]
    fn conflict_checked_writes() {
        let (dir, conn) = temp_vault("write");
        let events = se();
        let path = dir.join("Note.md");
        fs::write(&path, "v1").unwrap();
        note_index::index_note(&conn, "Note", &path.to_string_lossy(), "v1").unwrap();
        let base = content_hash("v1");

        // Normal save against the expected disk state.
        let r = write_note(&conn, &dir, &events, "Note", "v2", Some(&base)).unwrap();
        assert_eq!(r.status, "saved");
        assert_eq!(r.hash, content_hash("v2"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "v2");

        // External edit lands; a save against the stale base must not win.
        fs::write(&path, "external").unwrap();
        let r = write_note(
            &conn,
            &dir,
            &events,
            "Note",
            "v3",
            Some(&content_hash("v2")),
        )
        .unwrap();
        assert_eq!(r.status, "conflict");
        assert_eq!(r.disk, "external");
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");

        // Idempotent save: disk already equals what we're writing.
        let r = write_note(&conn, &dir, &events, "Note", "external", Some(&base)).unwrap();
        assert_eq!(r.status, "saved");

        // External deletion: autosave must not silently recreate the file.
        fs::remove_file(&path).unwrap();
        let r = write_note(&conn, &dir, &events, "Note", "v4", Some(&base)).unwrap();
        assert_eq!(r.status, "missing");
        assert!(!path.exists());

        // Deliberate force-write (user chose "keep my version") recreates it.
        let r = write_note(&conn, &dir, &events, "Note", "v4", None).unwrap();
        assert_eq!(r.status, "saved");
        assert_eq!(fs::read_to_string(&path).unwrap(), "v4");

        fs::remove_dir_all(&dir).ok();
    }

    fn build_rename_vault(tag: &str) -> (PathBuf, Connection) {
        let (dir, conn) = temp_vault(tag);
        fs::write(
            dir.join("Plinth Roadmap.md"),
            "# Plinth Roadmap\n\nShip v0.3.0. See [[Plinth Roadmap]] notes. #roadmap #dev\n",
        )
        .unwrap();
        fs::write(
            dir.join("2026-07-03.md"),
            "# 2026-07-03\n\nWorked on [[plinth roadmap]] today. Plinth Roadmap as prose stays. #daily\n",
        )
        .unwrap();
        fs::write(
            dir.join("Ideas.md"),
            "# Ideas\n\nLink: [[ Plinth Roadmap ]] with padding, and [[Someday]]. #ideas\n",
        )
        .unwrap();
        fs::write(dir.join("Unrelated.md"), "# Unrelated\n\nNo links here.\n").unwrap();
        let scan = note_index::reindex_vault(&conn, &dir).unwrap();
        assert_eq!(scan.notes_indexed, 4);
        (dir, conn)
    }

    #[test]
    fn rename_updates_files_links_and_index() {
        let (dir, conn) = build_rename_vault("rename");
        let events = se();
        note_index::touch_recent(&conn, "Plinth Roadmap").unwrap();

        let outcome = rename_note(&conn, &dir, &events, "Plinth Roadmap", "Roadmap 2026").unwrap();
        assert_eq!(outcome.name, "Roadmap 2026");
        // Three [[links]] rewritten: the self-link plus one in each source.
        assert_eq!(outcome.links_updated, 3);

        // The file itself moved; content (apart from links) is preserved.
        assert!(!dir.join("Plinth Roadmap.md").exists());
        let renamed = fs::read_to_string(dir.join("Roadmap 2026.md")).unwrap();
        assert!(renamed.contains("Ship v0.3.0."));
        assert!(renamed.contains("[[Roadmap 2026]]"));

        // Incoming links rewritten regardless of casing or padding; plain
        // prose mentioning the old name is untouched.
        let daily = fs::read_to_string(dir.join("2026-07-03.md")).unwrap();
        assert!(daily.contains("[[Roadmap 2026]]"));
        assert!(daily.contains("Plinth Roadmap as prose stays."));
        assert!(!daily.contains("[[plinth roadmap]]"));
        let ideas = fs::read_to_string(dir.join("Ideas.md")).unwrap();
        assert!(ideas.contains("[[Roadmap 2026]]"));
        assert!(ideas.contains("[[Someday]]"));

        // Index: the old name is gone, the new one resolves, note count holds.
        assert!(note_index::note_path(&conn, "Plinth Roadmap")
            .unwrap()
            .is_none());
        let (path, canonical) = note_index::note_path(&conn, "roadmap 2026")
            .unwrap()
            .unwrap();
        assert_eq!(canonical, "Roadmap 2026");
        assert!(path.ends_with("Roadmap 2026.md"));
        assert_eq!(note_index::list_notes(&conn).unwrap().len(), 4);

        // Backlinks follow the rename (the self-link shows up too, as it
        // always has for self-linking notes).
        assert_eq!(
            note_index::backlinks(&conn, "Roadmap 2026").unwrap(),
            vec!["2026-07-03", "Ideas", "Roadmap 2026"]
        );
        assert!(note_index::backlinks(&conn, "Plinth Roadmap")
            .unwrap()
            .is_empty());

        // Tags stay attached to the renamed note.
        assert_eq!(
            note_index::notes_for_tag(&conn, "roadmap").unwrap(),
            vec!["Roadmap 2026"]
        );

        // Recents follow the rename.
        assert_eq!(
            note_index::recent_notes(&conn, 10).unwrap(),
            vec!["Roadmap 2026"]
        );

        // Search finds the new name and no longer surfaces the old one by title.
        let hits = note_index::search(&conn, "roadmap 2026").unwrap();
        assert_eq!(hits[0].name, "Roadmap 2026");

        // The graph carries the renamed node with its edges; no ghost of
        // the old name survives.
        let g = note_index::graph(&conn).unwrap();
        assert!(g.nodes.iter().any(|n| n.name == "Roadmap 2026" && n.exists));
        assert!(!g
            .nodes
            .iter()
            .any(|n| n.name.eq_ignore_ascii_case("plinth roadmap")));
        assert_eq!(
            g.edges
                .iter()
                .filter(|e| e.target == "Roadmap 2026")
                .count(),
            2
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rejections() {
        let (dir, conn) = build_rename_vault("reject");
        let events = se();

        // Collision with an existing note, case-insensitively.
        let err = rename_note(&conn, &dir, &events, "Ideas", "unrelated").unwrap_err();
        assert!(err.contains("already exists"));
        // Unsafe names.
        assert!(rename_note(&conn, &dir, &events, "Ideas", "a/b").is_err());
        assert!(rename_note(&conn, &dir, &events, "Ideas", "..").is_err());
        assert!(rename_note(&conn, &dir, &events, "Ideas", "").is_err());
        assert!(rename_note(&conn, &dir, &events, "Ideas", "   ").is_err());
        assert!(rename_note(&conn, &dir, &events, "Ideas", "con").is_err());
        assert!(rename_note(&conn, &dir, &events, "Ideas", ".plinth").is_err());
        // Renaming a note that doesn't exist.
        assert!(rename_note(&conn, &dir, &events, "Ghost", "Anything").is_err());
        // A no-op rename is reported, not silently accepted.
        assert!(rename_note(&conn, &dir, &events, "Ideas", "Ideas").is_err());

        // Nothing changed on disk or in the index.
        assert!(dir.join("Ideas.md").exists());
        assert!(dir.join("Unrelated.md").exists());
        assert_eq!(note_index::list_notes(&conn).unwrap().len(), 4);

        fs::remove_dir_all(&dir).ok();
    }

    /// SQLite's built-in NOCASE is ASCII-only, but Windows folds accented
    /// letters in paths too. The overridden Unicode collation must resolve
    /// [[über notes]] to "Über Notes" — otherwise the create-on-miss path
    /// would overwrite the real file with a stub (data loss).
    #[test]
    fn unicode_case_resolution_and_rename() {
        let (dir, conn) = temp_vault("unicode");
        let events = se();
        fs::write(
            dir.join("Über Notes.md"),
            "# Über Notes\n\nReal content that must survive.\n",
        )
        .unwrap();
        fs::write(dir.join("Daily.md"), "# Daily\n\nSee [[über notes]].\n").unwrap();
        note_index::reindex_vault(&conn, &dir).unwrap();

        // Lookup with non-ASCII case difference resolves to the real note.
        let (path, canonical) = note_index::note_path(&conn, "über notes").unwrap().unwrap();
        assert_eq!(canonical, "Über Notes");
        assert!(path.ends_with("Über Notes.md"));

        // A conflict-checked write through the lowercase name lands on the
        // existing file instead of stubbing over it.
        let base = content_hash("# Über Notes\n\nReal content that must survive.\n");
        let r = write_note(
            &conn,
            &dir,
            &events,
            "über notes",
            "# Über Notes\n\nEdited.\n",
            Some(&base),
        )
        .unwrap();
        assert_eq!(r.status, "saved");
        assert_eq!(
            fs::read_to_string(dir.join("Über Notes.md")).unwrap(),
            "# Über Notes\n\nEdited.\n"
        );

        // Backlinks see the non-ASCII-cased link, and a rename rewrites it.
        assert_eq!(
            note_index::backlinks(&conn, "Über Notes").unwrap(),
            vec!["Daily"]
        );
        let outcome = rename_note(&conn, &dir, &events, "Über Notes", "Uber Archive").unwrap();
        assert_eq!(outcome.links_updated, 1);
        assert!(fs::read_to_string(dir.join("Daily.md"))
            .unwrap()
            .contains("[[Uber Archive]]"));
        assert!(!dir.join("Über Notes.md").exists());

        fs::remove_dir_all(&dir).ok();
    }

    /// Windows is case-insensitive: "ideas" -> "IDEAS" must rename the real
    /// file in place (not collide with itself) and update links to the new
    /// casing.
    #[test]
    fn case_only_rename() {
        let (dir, conn) = temp_vault("caseonly");
        let events = se();
        fs::write(dir.join("ideas.md"), "# ideas\n\ncontent\n").unwrap();
        fs::write(dir.join("Daily.md"), "# Daily\n\nSee [[Ideas]].\n").unwrap();
        note_index::reindex_vault(&conn, &dir).unwrap();

        let outcome = rename_note(&conn, &dir, &events, "ideas", "IDEAS").unwrap();
        assert_eq!(outcome.name, "IDEAS");

        // The on-disk filename now carries the new casing.
        let on_disk: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.to_lowercase() == "ideas.md")
            .collect();
        assert_eq!(on_disk, vec!["IDEAS.md"]);

        // Index and links adopt the new casing.
        let (_, canonical) = note_index::note_path(&conn, "ideas").unwrap().unwrap();
        assert_eq!(canonical, "IDEAS");
        assert!(fs::read_to_string(dir.join("Daily.md"))
            .unwrap()
            .contains("[[IDEAS]]"));
        assert_eq!(
            note_index::backlinks(&conn, "IDEAS").unwrap(),
            vec!["Daily"]
        );

        fs::remove_dir_all(&dir).ok();
    }
}
