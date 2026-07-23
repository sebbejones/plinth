mod link_parser;
mod note_index;
mod vault_ops;
mod watcher;

use note_index::{GraphData, NoteMeta, SearchHit, TagCount};
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};
use vault_ops::{content_hash, RenameOutcome, WriteResult};
use walkdir::WalkDir;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct NoteContent {
    pub name: String,
    pub content: String,
    pub hash: String,
}

/// Everything the frontend needs after opening a vault: the note list,
/// scan warnings (nested files, duplicate names), and whether filesystem
/// watching could be started.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct VaultInfo {
    pub notes: Vec<NoteMeta>,
    pub warnings: Vec<String>,
    pub watcher_error: Option<String>,
}

struct Vault {
    root: PathBuf,
    conn: Connection,
    generation: u64,
    // Kept alive for the vault's lifetime; dropping it stops the watch.
    _watcher: Option<notify::RecommendedWatcher>,
}

struct AppState {
    vault: Arc<Mutex<Option<Vault>>>,
    self_events: Arc<Mutex<watcher::SelfEvents>>,
}

static VAULT_GENERATION: AtomicU64 = AtomicU64::new(1);

fn with_vault<T>(
    state: &State<AppState>,
    f: impl FnOnce(&mut Vault) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
    match guard.as_mut() {
        Some(vault) => f(vault),
        None => Err("No vault selected".into()),
    }
}

#[tauri::command]
fn set_vault(
    path: String,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<VaultInfo, String> {
    let root = PathBuf::from(&path);
    vault_ops::validate_vault_root(&root)?;
    if !root.is_dir() {
        return Err(format!(
            "The folder no longer exists (or isn't a folder): {path}"
        ));
    }
    let cache_dir = root.join(".plinth");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let conn = note_index::open(&cache_dir.join("index.db")).map_err(|e| e.to_string())?;
    let scan = note_index::reindex_vault(&conn, &root)?;
    let notes = note_index::list_notes(&conn).map_err(|e| e.to_string())?;

    let generation = VAULT_GENERATION.fetch_add(1, Ordering::SeqCst);
    let mut guard = state.vault.lock().map_err(|e| e.to_string())?;
    *guard = Some(Vault {
        root: root.clone(),
        conn,
        generation,
        _watcher: None,
    });

    // Watcher startup failure is survivable: the vault still opens, the
    // frontend just learns that outside edits won't be noticed live.
    let vault_arc = state.vault.clone();
    let self_events = state.self_events.clone();
    let process = Box::new(move |paths: std::collections::BTreeSet<PathBuf>| {
        let guard = vault_arc.lock().ok()?;
        let vault = guard.as_ref()?;
        if vault.generation != generation {
            return None; // A different vault is open now; stale batch.
        }
        Some(watcher::sync_paths(
            &vault.conn,
            &vault.root,
            &paths,
            &self_events,
        ))
    });
    let emit_handle = app.clone();
    let emit = Box::new(move |event: &str, payload: serde_json::Value| {
        let _ = emit_handle.emit(event, payload);
    });
    let watcher_error = match watcher::start_watcher(root, process, emit) {
        Ok(w) => {
            if let Some(vault) = guard.as_mut() {
                vault._watcher = Some(w);
            }
            None
        }
        Err(e) => Some(e),
    };
    drop(guard);

    Ok(VaultInfo {
        notes,
        warnings: scan.warnings,
        watcher_error,
    })
}

#[tauri::command]
fn read_dir(state: State<AppState>) -> Result<Vec<NoteMeta>, String> {
    with_vault(&state, |v| {
        note_index::list_notes(&v.conn).map_err(|e| e.to_string())
    })
}

/// Read a note by name. Following a [[link]] to a note that does not exist
/// yet should create it, so a miss creates the file with a heading stub.
/// Returns the canonical name so "[[plinth roadmap]]" opens as "Plinth Roadmap".
#[tauri::command]
fn read_file(name: String, state: State<AppState>) -> Result<NoteContent, String> {
    let name = name.trim().to_string();
    vault_ops::validate_note_name(&name)?;
    let self_events = state.self_events.clone();
    with_vault(&state, |v| {
        let result = match note_index::note_path(&v.conn, &name).map_err(|e| e.to_string())? {
            Some((path, canonical)) => {
                let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                NoteContent {
                    hash: content_hash(&content),
                    name: canonical,
                    content,
                }
            }
            None => {
                let path = v.root.join(format!("{name}.md"));
                let content = format!("# {name}\n\n");
                if let Ok(mut se) = self_events.lock() {
                    se.note_write(&path, &content_hash(&content));
                }
                fs::write(&path, &content).map_err(|e| e.to_string())?;
                note_index::index_note(&v.conn, &name, &path.to_string_lossy(), &content)
                    .map_err(|e| e.to_string())?;
                NoteContent {
                    hash: content_hash(&content),
                    name: name.clone(),
                    content,
                }
            }
        };
        note_index::touch_recent(&v.conn, &result.name).map_err(|e| e.to_string())?;
        Ok(result)
    })
}

/// Read a note only if its file exists — never creates, never touches
/// recents. Used to reload after external changes and to inspect conflicts.
#[tauri::command]
fn load_note(name: String, state: State<AppState>) -> Result<Option<NoteContent>, String> {
    let name = name.trim().to_string();
    vault_ops::validate_note_name(&name)?;
    with_vault(&state, |v| {
        match note_index::note_path(&v.conn, &name).map_err(|e| e.to_string())? {
            Some((path, canonical)) => match fs::read_to_string(&path) {
                Ok(content) => Ok(Some(NoteContent {
                    hash: content_hash(&content),
                    name: canonical,
                    content,
                })),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            },
            None => Ok(None),
        }
    })
}

/// Save a note without ever silently clobbering an external edit. The
/// caller passes the hash of the disk content it last saw; see
/// `vault_ops::write_note` for the saved/conflict/missing outcomes.
#[tauri::command]
fn write_file(
    name: String,
    content: String,
    base_hash: Option<String>,
    state: State<AppState>,
) -> Result<WriteResult, String> {
    let name = name.trim().to_string();
    let self_events = state.self_events.clone();
    with_vault(&state, |v| {
        vault_ops::write_note(
            &v.conn,
            &v.root,
            &self_events,
            &name,
            &content,
            base_hash.as_deref(),
        )
    })
}

/// Rename a note's file and rewrite every [[link]] pointing at it.
#[tauri::command]
fn rename_note(
    old_name: String,
    new_name: String,
    state: State<AppState>,
) -> Result<RenameOutcome, String> {
    let self_events = state.self_events.clone();
    with_vault(&state, |v| {
        vault_ops::rename_note(&v.conn, &v.root, &self_events, &old_name, &new_name)
    })
}

/// Delete a note's file and drop it from the index. Notes that linked to it
/// keep their [[link]] text; those links just create the note again if clicked.
#[tauri::command]
fn delete_file(name: String, state: State<AppState>) -> Result<Vec<NoteMeta>, String> {
    let name = name.trim().to_string();
    vault_ops::validate_note_name(&name)?;
    let self_events = state.self_events.clone();
    with_vault(&state, |v| {
        if let Some((path, _)) = note_index::note_path(&v.conn, &name).map_err(|e| e.to_string())? {
            if let Ok(mut se) = self_events.lock() {
                se.note_delete(&path);
            }
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        note_index::remove_note(&v.conn, &name).map_err(|e| e.to_string())?;
        note_index::list_notes(&v.conn).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn get_recents(state: State<AppState>) -> Result<Vec<String>, String> {
    with_vault(&state, |v| {
        note_index::recent_notes(&v.conn, 10).map_err(|e| e.to_string())
    })
}

/// Zip every visible file in the vault to `dest`. Returns the file count.
#[tauri::command]
fn export_vault(dest: String, state: State<AppState>) -> Result<usize, String> {
    with_vault(&state, |v| export_zip(&v.root, Path::new(&dest)))
}

fn export_zip(root: &Path, dest: &Path) -> Result<usize, String> {
    let file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !note_index::is_hidden(e))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        // Don't zip the archive into itself if it's being written into the vault.
        if !path.is_file() || path == dest {
            continue;
        }
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
        zip.start_file(rel.to_string_lossy().replace('\\', "/"), options)
            .map_err(|e| e.to_string())?;
        let data = fs::read(path).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
        count += 1;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
fn search(query: String, state: State<AppState>) -> Result<Vec<SearchHit>, String> {
    with_vault(&state, |v| {
        note_index::search(&v.conn, &query).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn get_backlinks(name: String, state: State<AppState>) -> Result<Vec<String>, String> {
    with_vault(&state, |v| {
        note_index::backlinks(&v.conn, name.trim()).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn get_graph(state: State<AppState>) -> Result<GraphData, String> {
    with_vault(&state, |v| {
        note_index::graph(&v.conn).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn get_tags(state: State<AppState>) -> Result<Vec<TagCount>, String> {
    with_vault(&state, |v| {
        note_index::all_tags(&v.conn).map_err(|e| e.to_string())
    })
}

#[tauri::command]
fn get_notes_by_tag(tag: String, state: State<AppState>) -> Result<Vec<String>, String> {
    with_vault(&state, |v| {
        note_index::notes_for_tag(&v.conn, &tag).map_err(|e| e.to_string())
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            vault: Arc::new(Mutex::new(None)),
            self_events: Arc::new(Mutex::new(watcher::SelfEvents::default())),
        })
        .invoke_handler(tauri::generate_handler![
            set_vault,
            read_dir,
            read_file,
            load_note,
            write_file,
            rename_note,
            delete_file,
            search,
            get_backlinks,
            get_graph,
            get_tags,
            get_notes_by_tag,
            get_recents,
            export_vault
        ])
        .run(tauri::generate_context!())
        .expect("error while running Plinth");
}
