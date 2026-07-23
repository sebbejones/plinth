//! Filesystem watching: notice Markdown files created, changed, deleted or
//! renamed outside Plinth while a vault is open, and keep the SQLite index
//! in agreement with the disk.
//!
//! The vault model is flat (root-level `*.md` only), so the watch itself is
//! non-recursive — events inside `.plinth` or any subfolder never even
//! reach the filter. Plinth's own saves are recognised by content hash via
//! `SelfEvents` and skipped, so autosave never loops through the watcher.

use crate::note_index;
use crate::vault_ops::content_hash;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a burst of events may stay quiet before it is processed, and
/// the hard cap on how long processing may be deferred while events keep
/// arriving.
const QUIET_MS: u64 = 300;
const MAX_BATCH_MS: u64 = 1500;

/// How long a record of Plinth's own write/delete stays valid. Long enough
/// to outlive slow watcher delivery, short enough that a genuinely external
/// edit landing on the same content later is still noticed elsewhere.
const SELF_EVENT_TTL: Duration = Duration::from_secs(15);

fn norm(path: &Path) -> String {
    // Windows paths compare case-insensitively; trailing separators are noise.
    path.to_string_lossy()
        .to_lowercase()
        .trim_end_matches(['\\', '/'])
        .to_string()
}

/// Paths Plinth itself just wrote or deleted, so the watcher can tell its
/// own disk activity apart from external programs'.
#[derive(Default)]
pub struct SelfEvents {
    writes: Vec<(String, String, Instant)>, // (normalized path, content hash, when)
    deletes: Vec<(String, Instant)>,
}

impl SelfEvents {
    fn prune(&mut self) {
        let now = Instant::now();
        self.writes
            .retain(|(_, _, t)| now.duration_since(*t) < SELF_EVENT_TTL);
        self.deletes
            .retain(|(_, t)| now.duration_since(*t) < SELF_EVENT_TTL);
    }

    pub fn note_write(&mut self, path: &Path, hash: &str) {
        self.prune();
        let key = norm(path);
        // A write supersedes any pending delete record for the path, so a
        // delete-then-recreate inside the TTL can't mask a later genuinely
        // external deletion of the recreated file.
        self.deletes.retain(|(p, _)| p != &key);
        self.writes.retain(|(p, _, _)| p != &key);
        self.writes.push((key, hash.to_string(), Instant::now()));
    }

    pub fn note_delete(&mut self, path: &Path) {
        self.prune();
        let key = norm(path);
        self.deletes.retain(|(p, _)| p != &key);
        self.deletes.push((key, Instant::now()));
    }

    /// True when `content` on disk is exactly what Plinth last wrote there.
    pub fn matches_write(&mut self, path: &Path, hash: &str) -> bool {
        self.prune();
        let key = norm(path);
        self.writes.iter().any(|(p, h, _)| p == &key && h == hash)
    }

    pub fn matches_delete(&mut self, path: &Path) -> bool {
        self.prune();
        let key = norm(path);
        self.deletes.iter().any(|(p, _)| p == &key)
    }
}

/// Is this a path the index cares about? Root-level Markdown only; hidden
/// files, editor temp files (`~$…`), and everything in subfolders
/// (including `.plinth`) are irrelevant.
pub fn relevant(root: &Path, path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if norm(parent) != norm(root) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if name.starts_with('.') || name.starts_with("~$") {
        return false;
    }
    let lower = name.to_lowercase();
    lower.ends_with(".md") && lower.len() > 3
}

/// What one processed batch of filesystem events changed, in note names.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ChangeSet {
    pub created: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub warnings: Vec<String>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty()
            && self.modified.is_empty()
            && self.deleted.is_empty()
            && self.warnings.is_empty()
    }
}

/// Bring the index in line with the disk for every path in the batch.
/// Existing files are (re)indexed, missing files drop out of the index; an
/// external rename arrives as one deletion plus one creation.
pub fn sync_paths(
    conn: &Connection,
    root: &Path,
    paths: &BTreeSet<PathBuf>,
    self_events: &Mutex<SelfEvents>,
) -> ChangeSet {
    let mut changes = ChangeSet::default();
    for path in paths {
        sync_path(conn, root, path, self_events, &mut changes);
    }
    changes
}

fn sync_path(
    conn: &Connection,
    root: &Path,
    path: &Path,
    self_events: &Mutex<SelfEvents>,
    changes: &mut ChangeSet,
) {
    if !relevant(root, path) {
        return;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    match fs::read_to_string(path) {
        Ok(content) => {
            let hash = content_hash(&content);
            if let Ok(mut se) = self_events.lock() {
                if se.matches_write(path, &hash) {
                    return; // Plinth's own save echoing back.
                }
            }
            let existing = match note_index::note_path(conn, stem) {
                Ok(e) => e,
                Err(e) => {
                    changes.warnings.push(e.to_string());
                    return;
                }
            };
            match existing {
                Some((known_path, canonical)) if norm(&known_path) != norm(path) => {
                    if known_path.exists() {
                        // Two on-disk files share one note name — refuse to
                        // pick a winner silently.
                        changes.warnings.push(format!(
                            "\"{}\" clashes with the existing note \"{canonical}\" (names \
                             match ignoring case). The new file was not indexed — rename it \
                             outside Plinth.",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                        return;
                    }
                    // The old file is gone (e.g. an external case-only or
                    // cross-case rename): adopt the new file under its
                    // on-disk name.
                    let _ = note_index::remove_note(conn, &canonical);
                    if index_ok(conn, stem, path, &content, changes) {
                        changes.modified.push(stem.to_string());
                    }
                }
                Some((_, canonical)) => {
                    if index_ok(conn, &canonical, path, &content, changes) {
                        changes.modified.push(canonical);
                    }
                }
                None => {
                    if index_ok(conn, stem, path, &content, changes) {
                        changes.created.push(stem.to_string());
                    }
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Ok(mut se) = self_events.lock() {
                if se.matches_delete(path) {
                    return; // Plinth's own delete echoing back.
                }
            }
            if let Ok(Some((known_path, canonical))) = note_index::note_path(conn, stem) {
                if norm(&known_path) == norm(path) {
                    let _ = note_index::remove_note(conn, &canonical);
                    changes.deleted.push(canonical);
                }
            }
        }
        Err(_) => {
            // Probably a transient lock from the external editor mid-write;
            // a follow-up event will arrive when the write completes.
        }
    }
}

fn index_ok(
    conn: &Connection,
    name: &str,
    path: &Path,
    content: &str,
    changes: &mut ChangeSet,
) -> bool {
    match note_index::index_note(conn, name, &path.to_string_lossy(), content) {
        Ok(()) => true,
        Err(e) => {
            changes.warnings.push(e.to_string());
            false
        }
    }
}

enum Msg {
    Paths(Vec<PathBuf>),
    Error(String),
}

/// Start watching `root` (non-recursively). Relevant events are debounced
/// into batches, handed to `process`, and the resulting `ChangeSet` is
/// pushed to the frontend through `emit`. Watcher runtime errors surface
/// once as a `watcher-error` event instead of crashing anything.
///
/// The returned watcher must be kept alive; dropping it stops the watch and
/// lets the worker thread exit.
pub fn start_watcher(
    root: PathBuf,
    process: Box<dyn Fn(BTreeSet<PathBuf>) -> Option<ChangeSet> + Send>,
    emit: Box<dyn Fn(&str, serde_json::Value) + Send>,
) -> Result<notify::RecommendedWatcher, String> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let cb_root = root.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                let paths: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|p| relevant(&cb_root, p))
                    .collect();
                if !paths.is_empty() {
                    let _ = tx.send(Msg::Paths(paths));
                }
            }
            Err(e) => {
                let _ = tx.send(Msg::Error(e.to_string()));
            }
        })
        .map_err(|e| e.to_string())?;
    watcher
        .watch(&root, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    std::thread::spawn(move || {
        let mut reported_error = false;
        loop {
            let first = match rx.recv() {
                Ok(msg) => msg,
                Err(_) => return, // Watcher dropped; vault closed or replaced.
            };
            let mut pending: BTreeSet<PathBuf> = BTreeSet::new();
            match first {
                Msg::Paths(ps) => pending.extend(ps),
                Msg::Error(e) => {
                    if !reported_error {
                        reported_error = true;
                        emit("watcher-error", serde_json::Value::String(e));
                    }
                    continue;
                }
            }
            // Coalesce the burst: wait for a quiet gap, but never defer
            // longer than MAX_BATCH_MS while events keep streaming in.
            let deadline = Instant::now() + Duration::from_millis(MAX_BATCH_MS);
            loop {
                match rx.recv_timeout(Duration::from_millis(QUIET_MS)) {
                    Ok(Msg::Paths(ps)) => {
                        pending.extend(ps);
                        if Instant::now() >= deadline {
                            break;
                        }
                    }
                    Ok(Msg::Error(e)) => {
                        if !reported_error {
                            reported_error = true;
                            emit("watcher-error", serde_json::Value::String(e));
                        }
                    }
                    Err(_) => break, // Quiet, or channel closed — process what we have.
                }
            }
            if let Some(changes) = process(pending) {
                if !changes.is_empty() {
                    if let Ok(payload) = serde_json::to_value(&changes) {
                        emit("vault-changed", payload);
                    }
                }
            }
        }
    });
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_index;
    use std::sync::Arc;

    fn temp_vault(tag: &str) -> (PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("plinth-watch-{tag}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(dir.join(".plinth")).unwrap();
        let conn = note_index::open(&dir.join(".plinth/index.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn relevance_filter() {
        let root = Path::new("C:/v");
        assert!(relevant(root, Path::new("C:/v/Note.md")));
        assert!(relevant(root, Path::new("C:/v/Note.MD")));
        // Subfolders, .plinth, hidden and temp files, other types: ignored.
        assert!(!relevant(root, Path::new("C:/v/sub/Note.md")));
        assert!(!relevant(root, Path::new("C:/v/.plinth/index.db")));
        assert!(!relevant(root, Path::new("C:/v/.plinth/Fake.md")));
        assert!(!relevant(root, Path::new("C:/v/.hidden.md")));
        assert!(!relevant(root, Path::new("C:/v/~$Note.md")));
        assert!(!relevant(root, Path::new("C:/v/notes.txt")));
        assert!(!relevant(root, Path::new("C:/v/export.zip")));
        assert!(!relevant(root, Path::new("C:/v/Note.md.swp")));
        assert!(!relevant(root, Path::new("C:/other/Note.md")));
        // Case-insensitive root comparison, Windows-style.
        assert!(relevant(Path::new("C:/V"), Path::new("c:/v/Note.md")));
    }

    /// External create / modify / delete keep the index in line with disk;
    /// irrelevant files change nothing.
    #[test]
    fn sync_paths_mirrors_disk() {
        let (dir, conn) = temp_vault("sync");
        let se = Mutex::new(SelfEvents::default());
        fs::write(dir.join("Existing.md"), "# Existing\n\n[[Target]] #old\n").unwrap();
        note_index::reindex_vault(&conn, &dir).unwrap();

        // External creation.
        fs::write(
            dir.join("New Note.md"),
            "# New Note\n\nHello [[Existing]] #fresh\n",
        )
        .unwrap();
        let mut paths = BTreeSet::new();
        paths.insert(dir.join("New Note.md"));
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert_eq!(changes.created, vec!["New Note"]);
        assert!(note_index::note_path(&conn, "new note").unwrap().is_some());
        assert_eq!(
            note_index::backlinks(&conn, "Existing").unwrap(),
            vec!["New Note"]
        );
        assert!(note_index::all_tags(&conn)
            .unwrap()
            .iter()
            .any(|t| t.tag == "fresh"));

        // External modification.
        fs::write(
            dir.join("Existing.md"),
            "# Existing\n\nRewritten. #changed\n",
        )
        .unwrap();
        let mut paths = BTreeSet::new();
        paths.insert(dir.join("Existing.md"));
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert_eq!(changes.modified, vec!["Existing"]);
        let g = note_index::graph(&conn).unwrap();
        assert!(!g.nodes.iter().any(|n| n.name == "Target")); // old ghost gone
        assert!(note_index::all_tags(&conn)
            .unwrap()
            .iter()
            .any(|t| t.tag == "changed"));

        // External deletion.
        fs::remove_file(dir.join("New Note.md")).unwrap();
        let mut paths = BTreeSet::new();
        paths.insert(dir.join("New Note.md"));
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert_eq!(changes.deleted, vec!["New Note"]);
        assert!(note_index::note_path(&conn, "new note").unwrap().is_none());

        // Irrelevant files produce no changes at all.
        fs::write(dir.join("notes.txt"), "text").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/Nested.md"), "# Nested\n").unwrap();
        fs::write(dir.join(".plinth/scratch.md"), "# X\n").unwrap();
        let mut paths = BTreeSet::new();
        paths.insert(dir.join("notes.txt"));
        paths.insert(dir.join("sub/Nested.md"));
        paths.insert(dir.join(".plinth/scratch.md"));
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert!(changes.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    /// Plinth's own saves and deletes echo back from the OS watcher; the
    /// self-event record keeps them from being reprocessed or looping.
    #[test]
    fn self_events_are_suppressed() {
        let (dir, conn) = temp_vault("self");
        let se = Mutex::new(SelfEvents::default());
        let path = dir.join("Mine.md");
        let content = "# Mine\n\nSaved by Plinth.\n";
        fs::write(&path, content).unwrap();
        note_index::reindex_vault(&conn, &dir).unwrap();
        se.lock().unwrap().note_write(&path, &content_hash(content));

        let mut paths = BTreeSet::new();
        paths.insert(path.clone());
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert!(changes.is_empty(), "own save must not re-emit changes");

        // But a genuinely different external edit right after our save is
        // still noticed — suppression is by content, not by time alone.
        fs::write(&path, "# Mine\n\nEdited outside.\n").unwrap();
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert_eq!(changes.modified, vec!["Mine"]);

        // Same for deletion.
        se.lock().unwrap().note_delete(&path);
        fs::remove_file(&path).unwrap();
        let changes = sync_paths(&conn, &dir, &paths, &se);
        assert!(changes.is_empty(), "own delete must not re-emit changes");
        // The index entry was already removed by the delete command in real
        // use; here it simply stays, which is what "suppressed" means.

        fs::remove_dir_all(&dir).ok();
    }

    /// A Plinth delete followed by a recreate must not leave a suppression
    /// record that would mask a later genuinely external deletion.
    #[test]
    fn recreate_clears_delete_suppression() {
        let mut se = SelfEvents::default();
        let path = Path::new("C:/v/Note.md");
        se.note_delete(path);
        assert!(se.matches_delete(path));
        se.note_write(path, "somehash");
        assert!(
            !se.matches_delete(path),
            "write must supersede the delete record"
        );
    }

    /// End-to-end through a real OS watcher: an external write leads to a
    /// vault-changed emission and an updated index.
    #[test]
    fn watcher_pipeline_end_to_end() {
        let (dir, conn) = temp_vault("e2e");
        fs::write(dir.join("Seed.md"), "# Seed\n").unwrap();
        note_index::reindex_vault(&conn, &dir).unwrap();

        let conn = Arc::new(Mutex::new(conn));
        let se = Arc::new(Mutex::new(SelfEvents::default()));
        let (tx, rx) = mpsc::channel::<(String, serde_json::Value)>();

        let p_conn = conn.clone();
        let p_root = dir.clone();
        let p_se = se.clone();
        let process = Box::new(move |paths: BTreeSet<PathBuf>| {
            let conn = p_conn.lock().unwrap();
            Some(sync_paths(&conn, &p_root, &paths, &p_se))
        });
        let emit = Box::new(move |name: &str, payload: serde_json::Value| {
            let _ = tx.send((name.to_string(), payload));
        });
        let _watcher = start_watcher(dir.clone(), process, emit).expect("watcher starts");

        // Give the OS watch a moment to arm, then edit externally.
        std::thread::sleep(Duration::from_millis(300));
        fs::write(
            dir.join("Arrival.md"),
            "# Arrival\n\nWritten by another app. #ext\n",
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut created_seen = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok((name, payload)) if name == "vault-changed" => {
                    let all = format!("{payload}");
                    if all.contains("Arrival") {
                        created_seen = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(
            created_seen,
            "external creation must surface as vault-changed"
        );
        assert!(note_index::note_path(&conn.lock().unwrap(), "Arrival")
            .unwrap()
            .is_some());

        // Deletion flows through as well.
        fs::remove_file(dir.join("Arrival.md")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut deleted_seen = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok((name, payload)) if name == "vault-changed" => {
                    if format!("{payload}").contains("\"Deleted\":[\"Arrival\"]") {
                        deleted_seen = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(
            deleted_seen,
            "external deletion must surface as vault-changed"
        );
        assert!(note_index::note_path(&conn.lock().unwrap(), "Arrival")
            .unwrap()
            .is_none());

        fs::remove_dir_all(&dir).ok();
    }
}
