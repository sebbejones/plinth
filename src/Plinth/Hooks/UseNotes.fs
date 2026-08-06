/// All vault/note state management in one custom hook.
module Plinth.Hooks.UseNotes

open Browser.WebStorage
open Fable.Core
open Feliz
open Plinth
open Plinth.Types
open Plinth.Utils

type NotesApi =
    { Vault: string option
      Notes: NoteMeta[]
      Tags: TagCount[]
      Backlinks: string[]
      Recents: string[]
      Current: NoteState
      TagFilter: (string * string[]) option
      Dirty: bool
      /// Things to act on: duplicate note names, index write failures.
      Warnings: string[]
      /// Things working as designed that are still worth saying once —
      /// Markdown left alone in subfolders, an existing sample reopened.
      Notices: string[]
      /// Set when filesystem watching is unavailable for the open vault.
      WatcherError: string option
      /// Why the app is on the welcome screen (e.g. last vault missing).
      WelcomeInfo: string option
      /// Pick a folder and open it as a vault. Used for "change vault"
      /// and for the welcome screen's "open an existing folder".
      PickVault: unit -> unit
      /// Same picker, worded for someone starting from nothing.
      StartNewVault: unit -> unit
      /// Seed (or reopen) the sample notebook in Documents and open it.
      OpenSampleVault: unit -> unit
      OpenNote: string -> unit
      OpenToday: unit -> unit
      UpdateContent: string -> unit
      /// Flush the pending autosave right now (Ctrl+S).
      SaveNow: unit -> unit
      DeleteNote: string -> unit
      /// Rename the current note. Resolves to Some error when it failed.
      RenameNote: string -> JS.Promise<string option>
      /// In a conflict/deleted state: write the local text to disk.
      KeepLocalVersion: unit -> unit
      /// In a conflict: adopt the disk version, discarding local edits.
      LoadDiskVersion: unit -> unit
      /// In a deleted state: accept the deletion and navigate away.
      DiscardLocal: unit -> unit
      DismissWarnings: unit -> unit
      DismissNotices: unit -> unit
      DismissWatcherError: unit -> unit
      ExportVault: unit -> unit
      FilterByTag: string -> unit
      ClearTagFilter: unit -> unit }

let private lastVaultKey = "plinth-last-vault"

let useNotes () : NotesApi =
    let vault, setVault = React.useState<string option> None
    let notes, setNotes = React.useState<NoteMeta[]> [||]
    let tags, setTags = React.useState<TagCount[]> [||]
    let backlinks, setBacklinks = React.useState<string[]> [||]
    let recents, setRecents = React.useState<string[]> [||]
    let current, setCurrent = React.useState<NoteState> NoVault
    let tagFilter, setTagFilter = React.useState<(string * string[]) option> None
    let dirty, setDirty = React.useState false
    let warnings, setWarnings = React.useStateWithUpdater<string[]> [||]
    let notices, setNotices = React.useStateWithUpdater<string[]> [||]
    let watcherError, setWatcherError = React.useState<string option> None
    let welcomeInfo, setWelcomeInfo = React.useState<string option> None
    // Bumped on every keystroke so a save that finishes can tell whether
    // newer edits arrived while it was writing (and must stay dirty).
    let editGen = React.useRef 0
    // Hash of the disk content the editor last loaded or saved — the save
    // command uses it to detect external edits instead of overwriting them.
    let baseHash = React.useRef<string option> None
    // Live mirrors for the once-registered filesystem event listener.
    let currentRef = React.useRef current
    let dirtyRef = React.useRef dirty
    currentRef.current <- current
    dirtyRef.current <- dirty

    let refreshTags () =
        promise {
            let! t = Tauri.getTags ()
            setTags t
        }

    let refreshRecents () =
        promise {
            let! r = Tauri.getRecents ()
            setRecents r
        }

    let openNote (name: string) =
        let name = name.Trim()
        setCurrent (LoadingNote name)

        promise {
            try
                let! note = Tauri.readFile name
                // The note may have just been created by following a link,
                // so refresh the sidebar list too.
                let! updated = Tauri.readDir ()
                setNotes updated
                let! bl = Tauri.getBacklinks note.Name
                setBacklinks bl
                do! refreshRecents ()
                do! refreshTags ()
                baseHash.current <- Some note.Hash
                setDirty false
                setCurrent (EditingNote(note.Name, note.Content))
            with ex ->
                setCurrent (NoteError(Tauri.errorText ex))
        }
        |> Promise.start

    let openToday () = openNote (Date.todayName ())

    /// Open a folder as the vault. `auto` marks the startup reopen of the
    /// remembered vault: failures land on the welcome screen with an
    /// explanation instead of an error pane, and no note file is created
    /// unless the vault is empty. `landing` names the note to open when
    /// the caller has one in mind (the sample notebook's first page);
    /// `extra` is a notice to show alongside the vault's own.
    let openVaultPath (path: string) (auto: bool) (landing: string option) (extra: string list) =
        let before = current

        promise {
            try
                setCurrent VaultLoading
                let! info = Tauri.setVault path
                localStorage.setItem (lastVaultKey, path)
                setVault (Some path)
                setNotes info.Notes
                setWarnings (fun _ -> info.Warnings)
                setNotices (fun _ -> Array.append (List.toArray extra) info.Notices)
                setWatcherError info.WatcherError
                setTagFilter None
                setWelcomeInfo None

                // A landing note is only honoured if a file backs it. Asking
                // for one that isn't there would create it empty, which is
                // how someone who deleted a sample note gets it silently
                // resurrected as a blank page.
                let landing =
                    landing
                    |> Option.filter (fun n ->
                        info.Notes
                        |> Array.exists (fun x -> x.Name.ToLowerInvariant() = n.ToLowerInvariant()))

                match landing with
                | Some name -> openNote name
                | None when auto ->
                    // Land where the user left off rather than creating
                    // today's note as a side effect of starting the app.
                    let! recent = Tauri.getRecents ()
                    setRecents recent

                    match recent |> Array.tryHead with
                    | Some name -> openNote name
                    | None ->
                        match info.Notes |> Array.tryHead with
                        | Some n -> openNote n.Name
                        | None -> openToday ()
                | None -> openToday ()
            with ex ->
                match vault with
                | Some _ when not auto ->
                    // A vault is already open; report and stay where we were.
                    do! Tauri.messageDialog ("Couldn't open that folder as a vault: " + Tauri.errorText ex) true
                    setCurrent before
                | _ ->
                    setCurrent NoVault
                    setWelcomeInfo (
                        Some(
                            (if auto then
                                 sprintf "Couldn't reopen your last vault (%s): %s" path (Tauri.errorText ex)
                             else
                                 sprintf "Couldn't open that folder as a vault: %s" (Tauri.errorText ex))
                            + " Choose a vault folder to continue."
                        )
                    )
        }
        |> Promise.start

    let pickVaultTitled (title: string) =
        promise {
            let! choice = Tauri.pickFolder title

            match choice with
            | None -> ()
            | Some path -> openVaultPath path false None []
        }
        |> Promise.start

    let pickVault () =
        pickVaultTitled "Choose a folder of Markdown notes"

    /// Starting from nothing. The picker is the same one — on Windows it
    /// can make a folder in place — so the only thing that changes is the
    /// question being asked.
    let startNewVault () =
        pickVaultTitled "Choose an empty folder for your new notebook"

    /// The sample notebook. If one is already in Documents it is opened as
    /// the user left it rather than being rewritten under them; they are
    /// told that, once, instead of quietly getting stale-looking notes.
    let openSampleVault () =
        promise {
            try
                let! s = Tauri.createSampleVault (Date.todayName ())

                let extra =
                    if s.Created then
                        []
                    else
                        [ sprintf
                              "A sample notebook was already here, so Plinth opened it as you left it rather than overwriting anything. For a fresh copy, delete %s and try again."
                              s.Path ]

                openVaultPath s.Path false (Some "Start here") extra
            with ex ->
                setWelcomeInfo (Some("Couldn't create the sample notebook: " + Tauri.errorText ex))
        }
        |> Promise.start

    // Reopen the last vault on startup, if one was remembered.
    let reopenLastVault () =
        match localStorage.getItem lastVaultKey with
        | null
        | "" -> ()
        | path -> openVaultPath path true None []

    React.useEffect (reopenLastVault, [||])

    let updateContent (text: string) =
        match current with
        | EditingNote(name, _) ->
            editGen.current <- editGen.current + 1
            setDirty true
            setCurrent (EditingNote(name, text))
        | _ -> ()

    /// The one place a note is written. Both the debounced autosave and an
    /// explicit Ctrl+S come through here, so a save racing an external edit
    /// is handled identically no matter which triggered it.
    let performSave (name: string) (content: string) =
        let gen = editGen.current

        // The freshest local text at the time the save result arrives —
        // never park stale text in a conflict state.
        let latestLocal () =
            match currentRef.current with
            | EditingNote(n, c) when n = name -> c
            | _ -> content

        promise {
            try
                let! result = Tauri.writeFile name content baseHash.current

                match result.Status with
                | "saved" ->
                    setNotes result.Notes
                    baseHash.current <- Some result.Hash
                    let! bl = Tauri.getBacklinks name
                    setBacklinks bl
                    do! refreshTags ()

                    if editGen.current = gen then
                        setDirty false
                | "conflict" -> setCurrent (ConflictNote(name, latestLocal (), result.Disk))
                | "missing" -> setCurrent (DeletedNote(name, Some(latestLocal ())))
                | _ -> ()
            with ex ->
                setCurrent (NoteError(Tauri.errorText ex))
        }
        |> Promise.start

    /// Ctrl+S. Autosave already guarantees the file lands shortly after the
    /// last keystroke, so this changes nothing about safety — it exists
    /// because the habit is universal and silence reads as data loss.
    let saveNow () =
        match currentRef.current with
        | EditingNote(name, content) -> performSave name content
        | _ -> ()

    // Debounced autosave, 700 ms after the last keystroke. The effect's
    // cleanup cancels the pending timer whenever the content changes again.
    // A save that discovers an external edit or deletion switches to the
    // matching conflict state instead of writing; autosave then stays off
    // because it only runs while the state is EditingNote.
    let autosaveEffect () : unit -> unit =
        match current, dirty with
        | EditingNote(name, content), true ->
            let timer = JS.setTimeout (fun () -> performSave name content) 700
            fun () -> JS.clearTimeout timer
        | _ -> ignore

    React.useEffect (autosaveEffect, [| box current; box dirty |])

    // React to external filesystem changes noticed by the Rust watcher.
    // Registered once; reads live state through refs.
    let externalChangesEffect () : unit -> unit =
        let mutable unlisteners: (unit -> unit) list = []
        let mutable disposed = false

        let nameOf state =
            match state with
            | EditingNote(n, _)
            | LoadingNote n
            | ConflictNote(n, _, _)
            | DeletedNote(n, _) -> Some n
            | _ -> None

        let touches (name: string) (arr: string[]) =
            let l = name.ToLowerInvariant()
            arr |> Array.exists (fun x -> x.ToLowerInvariant() = l)

        let handleChanges (changes: VaultChanges) =
            promise {
                let! updated = Tauri.readDir ()
                setNotes updated
                do! refreshTags ()
                do! refreshRecents ()

                if changes.Warnings.Length > 0 then
                    setWarnings (fun old ->
                        Array.append old (changes.Warnings |> Array.filter (fun w -> not (Array.contains w old))))

                match nameOf currentRef.current with
                | Some n ->
                    let! bl = Tauri.getBacklinks n
                    setBacklinks bl
                | None -> ()

                match currentRef.current with
                | EditingNote(name, content) ->
                    if touches name changes.Deleted then
                        setCurrent (DeletedNote(name, (if dirtyRef.current then Some content else None)))
                    elif touches name changes.Modified || touches name changes.Created then
                        let! disk = Tauri.loadNote name

                        match disk with
                        | Some d ->
                            if Some d.Hash = baseHash.current then
                                // Our own save echoing back; nothing changed.
                                ()
                            elif dirtyRef.current then
                                if d.Content = content then
                                    // The external write matches the local
                                    // text exactly — treat it as saved.
                                    baseHash.current <- Some d.Hash
                                    setDirty false
                                else
                                    setCurrent (ConflictNote(name, content, d.Content))
                            else
                                baseHash.current <- Some d.Hash
                                setCurrent (EditingNote(d.Name, d.Content))
                        | None ->
                            setCurrent (DeletedNote(name, (if dirtyRef.current then Some content else None)))
                | ConflictNote(name, local, _) ->
                    if touches name changes.Deleted then
                        setCurrent (DeletedNote(name, Some local))
                    elif touches name changes.Modified || touches name changes.Created then
                        // The disk side of the conflict moved again.
                        let! disk = Tauri.loadNote name

                        match disk with
                        | Some d -> setCurrent (ConflictNote(name, local, d.Content))
                        | None -> setCurrent (DeletedNote(name, Some local))
                | DeletedNote(name, local) ->
                    if touches name changes.Created || touches name changes.Modified then
                        // The file came back (recreated externally).
                        let! disk = Tauri.loadNote name

                        match disk, local with
                        | Some d, Some l when l <> d.Content -> setCurrent (ConflictNote(name, l, d.Content))
                        | Some d, _ ->
                            baseHash.current <- Some d.Hash
                            setDirty false
                            setCurrent (EditingNote(d.Name, d.Content))
                        | None, _ -> ()
                | _ -> ()
            }
            |> Promise.start

        promise {
            let! unChanged = Tauri.onVaultChanged handleChanges
            let! unError = Tauri.onWatcherError (fun msg -> setWatcherError (Some msg))

            if disposed then
                unChanged ()
                unError ()
            else
                unlisteners <- [ unChanged; unError ]
        }
        |> Promise.start

        fun () ->
            disposed <- true
            unlisteners |> List.iter (fun un -> un ())

    React.useEffect (externalChangesEffect, [||])

    let keepLocalVersion () =
        match current with
        | ConflictNote(name, local, _)
        | DeletedNote(name, Some local) ->
            promise {
                try
                    let! result = Tauri.writeFile name local None

                    if result.Status = "saved" then
                        setNotes result.Notes
                        baseHash.current <- Some result.Hash
                        setDirty false
                        setCurrent (EditingNote(name, local))
                        let! bl = Tauri.getBacklinks name
                        setBacklinks bl
                        do! refreshTags ()
                        do! refreshRecents ()
                with ex ->
                    setCurrent (NoteError(Tauri.errorText ex))
            }
            |> Promise.start
        | _ -> ()

    let loadDiskVersion () =
        match current with
        | ConflictNote(name, _, _) ->
            promise {
                try
                    let! disk = Tauri.loadNote name

                    match disk with
                    | Some d ->
                        baseHash.current <- Some d.Hash
                        setDirty false
                        setCurrent (EditingNote(d.Name, d.Content))
                    | None ->
                        // The disk version vanished while the user decided.
                        setCurrent (DeletedNote(name, None))
                with ex ->
                    setCurrent (NoteError(Tauri.errorText ex))
            }
            |> Promise.start
        | _ -> ()

    let discardLocal () =
        match current with
        | DeletedNote _ ->
            setDirty false

            promise {
                let! recent = Tauri.getRecents ()
                setRecents recent

                match recent |> Array.tryHead with
                | Some next -> openNote next
                | None ->
                    let! ns = Tauri.readDir ()

                    match ns |> Array.tryHead with
                    | Some n -> openNote n.Name
                    | None -> openToday ()
            }
            |> Promise.start
        | _ -> ()

    let renameNote (newName: string) : JS.Promise<string option> =
        promise {
            match current with
            | EditingNote(name, content) ->
                try
                    let mutable blocker = None

                    // Flush pending edits so the rename starts from a
                    // saved file — and surface any conflict instead.
                    if dirty then
                        let! flushed = Tauri.writeFile name content baseHash.current

                        match flushed.Status with
                        | "saved" ->
                            baseHash.current <- Some flushed.Hash
                            setNotes flushed.Notes
                            setDirty false
                        | "conflict" ->
                            setCurrent (ConflictNote(name, content, flushed.Disk))
                            blocker <- Some "The file changed on disk. Resolve the conflict first."
                        | "missing" ->
                            setCurrent (DeletedNote(name, Some content))
                            blocker <- Some "The file was deleted outside Plinth. Resolve that first."
                        | _ -> ()

                    match blocker with
                    | Some msg -> return Some msg
                    | None ->
                        let! outcome = Tauri.renameNote name newName
                        setNotes outcome.Notes
                        // Reload from disk: the rename may have rewritten a
                        // self-link inside this note's own body.
                        let! reloaded = Tauri.loadNote outcome.Name

                        match reloaded with
                        | Some d ->
                            baseHash.current <- Some d.Hash
                            setDirty false
                            setCurrent (EditingNote(d.Name, d.Content))
                        | None -> setCurrent (NoteError "The renamed note could not be reloaded.")

                        let! bl = Tauri.getBacklinks outcome.Name
                        setBacklinks bl
                        do! refreshTags ()
                        do! refreshRecents ()
                        return None
                with ex ->
                    return Some(Tauri.errorText ex)
            | _ -> return Some "Open a note to rename it."
        }

    let filterByTag (tag: string) =
        promise {
            let! names = Tauri.getNotesByTag tag
            setTagFilter (Some(tag, names))
        }
        |> Promise.start

    let deleteNote (name: string) =
        // Shared tail: the vault changed, so refresh everything and land
        // somewhere sensible rather than on a note that no longer exists.
        let settleAfterDelete (updated: NoteMeta[]) =
            promise {
                setNotes updated
                setTagFilter None
                do! refreshTags ()
                let! recent = Tauri.getRecents ()
                setRecents recent

                match recent |> Array.tryHead with
                | Some next -> openNote next
                | None ->
                    match updated |> Array.tryHead with
                    | Some n -> openNote n.Name
                    | None -> openToday ()
            }

        promise {
            try
                let! confirmed =
                    Tauri.confirmDialog (
                        sprintf "Move \"%s\" to the Recycle Bin?\n\nYou can restore it from there. Links to it in other notes stay as text and will recreate it if clicked." name
                    )

                if confirmed then
                    let! outcome = Tauri.deleteFile name false

                    if outcome.Status = "unsupported" then
                        // No Recycle Bin on this drive. Say so plainly and make
                        // the irreversible option a separate, explicit choice.
                        let! permanent =
                            Tauri.confirmDialog (
                                sprintf
                                    "This drive has no Recycle Bin, so \"%s\" can't be moved there. Nothing has been deleted yet.\n\nDelete it permanently instead? This cannot be undone."
                                    name
                            )

                        if permanent then
                            let! hard = Tauri.deleteFile name true
                            do! settleAfterDelete hard.Notes
                    else
                        do! settleAfterDelete outcome.Notes
            with ex ->
                setCurrent (NoteError(Tauri.errorText ex))
        }
        |> Promise.start

    let exportVault () =
        promise {
            try
                let! dest = Tauri.saveZipDialog (sprintf "plinth-vault-%s.zip" (Date.todayName ()))

                match dest with
                | None -> ()
                | Some path ->
                    let! count = Tauri.exportVault path
                    do! Tauri.messageDialog (sprintf "Exported %i files to:\n%s" count path) false
            with ex ->
                do! Tauri.messageDialog ("Export failed: " + Tauri.errorText ex) true
        }
        |> Promise.start

    { Vault = vault
      Notes = notes
      Tags = tags
      Backlinks = backlinks
      Recents = recents
      Current = current
      TagFilter = tagFilter
      Dirty = dirty
      Warnings = warnings
      Notices = notices
      WatcherError = watcherError
      WelcomeInfo = welcomeInfo
      PickVault = pickVault
      StartNewVault = startNewVault
      OpenSampleVault = openSampleVault
      OpenNote = openNote
      OpenToday = openToday
      UpdateContent = updateContent
      SaveNow = saveNow
      DeleteNote = deleteNote
      RenameNote = renameNote
      KeepLocalVersion = keepLocalVersion
      LoadDiskVersion = loadDiskVersion
      DiscardLocal = discardLocal
      DismissWarnings = fun () -> setWarnings (fun _ -> [||])
      DismissNotices = fun () -> setNotices (fun _ -> [||])
      DismissWatcherError = fun () -> setWatcherError None
      ExportVault = exportVault
      FilterByTag = filterByTag
      ClearTagFilter = fun () -> setTagFilter None }

