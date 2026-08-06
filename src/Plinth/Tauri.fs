/// The bridge to the Rust backend: every call the frontend can make.
module Plinth.Tauri

open Fable.Core
open Fable.Core.JsInterop
open Plinth.Types

[<Import("invoke", "@tauri-apps/api/core")>]
let private invoke<'T> (cmd: string) (args: obj) : JS.Promise<'T> = jsNative

/// Tauri commands reject with plain strings, not Error objects, so Fable's
/// `ex.Message` is undefined for them (and `Some undefined` erases to None,
/// silently swallowing the error). Normalize any caught value to text.
let errorText (ex: exn) : string =
    let raw: obj = box ex

    if jsTypeof raw = "string" then unbox raw
    else
        let msg: obj = box ex.Message
        if isNull msg || jsTypeof msg <> "string" then string raw else unbox msg

[<Import("open", "@tauri-apps/plugin-dialog")>]
let private openDialog (options: obj) : JS.Promise<string option> = jsNative

[<Import("save", "@tauri-apps/plugin-dialog")>]
let private saveDialogRaw (options: obj) : JS.Promise<string option> = jsNative

[<Import("confirm", "@tauri-apps/plugin-dialog")>]
let private confirmRaw (message: string) (options: obj) : JS.Promise<bool> = jsNative

[<Import("message", "@tauri-apps/plugin-dialog")>]
let private messageRaw (message: string) (options: obj) : JS.Promise<unit> = jsNative

[<Import("openUrl", "@tauri-apps/plugin-opener")>]
let private openUrlRaw (url: string) : JS.Promise<unit> = jsNative

[<Import("listen", "@tauri-apps/api/event")>]
let private listenRaw (event: string) (handler: {| payload: obj |} -> unit) : JS.Promise<unit -> unit> =
    jsNative

/// Ask the user to pick a folder. `title` carries the whole difference
/// between starting a new notebook and opening one that already exists —
/// the picker is the same either way. None if they cancelled.
let pickFolder (title: string) : JS.Promise<string option> =
    openDialog
        {| directory = true
           multiple = false
           title = title |}

/// Write the sample notebook into Documents (or find the one already
/// there). Does not open it; the caller runs `setVault` on the path.
let createSampleVault (today: string) : JS.Promise<SampleVault> =
    invoke "create_sample_vault" {| today = today |}

/// Hand a web link to the OS browser. The scheme is checked here as well as
/// in the renderer and in the capability scope: a note is user content, and
/// this is the one place it reaches outside the app.
let openExternal (url: string) : JS.Promise<unit> =
    let u = url.Trim()
    // Case-insensitive, like the renderer's regex — "HTTPS://" is a valid URL
    // and an exact-case check would drop it on the floor without a word.
    let lower = u.ToLowerInvariant()

    if lower.StartsWith("http://")
       || lower.StartsWith("https://")
       || lower.StartsWith("mailto:") then
        openUrlRaw u
    else
        Promise.lift ()

/// Native yes/no confirmation dialog.
let confirmDialog (message: string) : JS.Promise<bool> =
    confirmRaw message {| title = "Plinth"; kind = "warning" |}

/// Native info/error message box.
let messageDialog (message: string) (isError: bool) : JS.Promise<unit> =
    messageRaw message {| title = "Plinth"; kind = (if isError then "error" else "info") |}

/// Ask where to save the vault export. None if cancelled.
let saveZipDialog (defaultName: string) : JS.Promise<string option> =
    saveDialogRaw
        {| defaultPath = defaultName
           title = "Export vault as zip"
           filters = [| {| name = "Zip archive"; extensions = [| "zip" |] |} |] |}

/// Open a vault: rebuilds the SQLite index, starts the filesystem watcher,
/// and reports notes plus any scan warnings.
let setVault (path: string) : JS.Promise<VaultInfo> =
    invoke "set_vault" {| path = path |}

let readDir () : JS.Promise<NoteMeta[]> =
    invoke "read_dir" (createObj [])

/// Read a note; creates it if it doesn't exist yet. Returns the canonical
/// name (so "[[plinth roadmap]]" opens as "Plinth Roadmap") plus content.
let readFile (name: string) : JS.Promise<NoteContent> =
    invoke "read_file" {| name = name |}

/// Read a note only if its file exists — never creates it, never touches
/// recents. None when the file is gone. Used for reloads and conflicts.
let loadNote (name: string) : JS.Promise<NoteContent option> =
    invoke "load_note" {| name = name |}

/// Delete a note's file. By default it goes to the Recycle Bin; `permanent`
/// skips the bin, which is only for the retry after an "unsupported" result.
let deleteFile (name: string) (permanent: bool) : JS.Promise<DeleteOutcome> =
    invoke "delete_file" {| name = name; permanent = permanent |}

/// Save a note without clobbering external edits: `baseHash` is the hash of
/// the disk content last seen; pass None to write unconditionally (only
/// after the user explicitly chose to keep their version).
let writeFile (name: string) (content: string) (baseHash: string option) : JS.Promise<WriteResult> =
    invoke "write_file" {| name = name; content = content; baseHash = baseHash |}

/// Rename a note and rewrite every [[link]] to it across the vault.
let renameNote (oldName: string) (newName: string) : JS.Promise<RenameOutcome> =
    invoke "rename_note" {| oldName = oldName; newName = newName |}

/// External filesystem changes noticed by the watcher, debounced batches.
let onVaultChanged (handler: VaultChanges -> unit) : JS.Promise<unit -> unit> =
    listenRaw "vault-changed" (fun e -> handler (unbox e.payload))

/// The watcher failed at runtime; outside edits are no longer noticed.
let onWatcherError (handler: string -> unit) : JS.Promise<unit -> unit> =
    listenRaw "watcher-error" (fun e -> handler (unbox e.payload))

let search (query: string) : JS.Promise<SearchHit[]> =
    invoke "search" {| query = query |}

let getBacklinks (name: string) : JS.Promise<string[]> =
    invoke "get_backlinks" {| name = name |}

/// The whole vault as a graph for the Firmament view.
let getGraph () : JS.Promise<GraphData> =
    invoke "get_graph" (createObj [])

let getTags () : JS.Promise<TagCount[]> =
    invoke "get_tags" (createObj [])

let getNotesByTag (tag: string) : JS.Promise<string[]> =
    invoke "get_notes_by_tag" {| tag = tag |}

/// Last 10 opened notes, newest first.
let getRecents () : JS.Promise<string[]> =
    invoke "get_recents" (createObj [])

/// Zip the whole vault to `dest`; returns the number of files archived.
let exportVault (dest: string) : JS.Promise<int> =
    invoke "export_vault" {| dest = dest |}
