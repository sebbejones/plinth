/// Main layout: sidebar + editor + backlinks — or the welcome screen
/// when no vault has been chosen yet.
module Plinth.Pages.NoteView

open Browser.Dom
open Feliz
open Plinth
open Plinth.Types
open Plinth.Hooks
open Plinth.Hooks.UseSettings
open Plinth.Components

/// One of the three ways in. A card is the whole click target: the first
/// screen asks for a decision, so it should not also ask you to find the
/// button inside the explanation.
let private path (label: string) (text: string) (primary: bool) (onClick: unit -> unit) =
    let tone =
        if primary then
            "border-emerald-700 bg-emerald-800 text-white hover:bg-emerald-700 dark:border-emerald-600"
        else
            "border-stone-200 bg-white text-stone-800 hover:border-stone-300 hover:bg-stone-50 dark:border-stone-700 dark:bg-stone-800 dark:text-stone-200 dark:hover:bg-stone-700"

    Html.button [
        prop.className (
            "flex-1 rounded-lg border p-4 text-left shadow-sm transition-colors " + tone
        )
        prop.onClick (fun _ -> onClick ())
        prop.children [
            Html.h3 [ prop.className "font-semibold"; prop.text label ]
            Html.p [
                prop.className (
                    "mt-1 text-sm "
                    + (if primary then
                           "text-emerald-100"
                       else
                           "text-stone-500 dark:text-stone-400")
                )
                prop.text text
            ]
        ]
    ]

let private centered (children: ReactElement list) =
    Html.div [
        prop.className "flex flex-1 items-center justify-center text-stone-400 dark:text-stone-500"
        prop.children children
    ]

/// The first screen. It asks one question — where do your notes live? —
/// and gives three answers, including one for the person who does not yet
/// have an answer. The argument for Plinth is a footnote here; someone who
/// has just opened it for the first time has already been convinced enough
/// to download it, and what they need next is a way in.
let private welcome (info: string option) (onNew: unit -> unit) (onOpen: unit -> unit) (onSample: unit -> unit) =
    Html.div [
        prop.className "flex flex-1 flex-col items-center justify-center gap-8 px-8"
        prop.children [
            Html.div [
                prop.className "max-w-xl text-center"
                prop.children [
                    Html.h1 [
                        prop.className "font-serif text-5xl font-bold text-emerald-900 dark:text-emerald-300"
                        prop.text "Plinth"
                    ]
                    Html.p [
                        prop.className "mt-3 text-lg text-stone-600 dark:text-stone-300"
                        prop.text "Your notes are Markdown files in a folder you choose. Pick one to begin."
                    ]
                ]
            ]
            match info with
            | Some msg ->
                Html.div [
                    prop.className
                        "max-w-xl rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200"
                    prop.text msg
                ]
            | None -> Html.none
            Html.div [
                prop.className "flex w-full max-w-3xl flex-col gap-4 sm:flex-row sm:gap-6"
                prop.children [
                    path
                        "Create a new notebook"
                        "Choose an empty folder. Plinth opens today's note and you start typing."
                        true
                        onNew
                    path
                        "Open a folder I already have"
                        "Point Plinth at existing Markdown files. It reads them where they are and changes nothing."
                        false
                        onOpen
                    path
                        "Try a sample notebook"
                        "A handful of real notes in your Documents folder, to click around in first."
                        false
                        onSample
                ]
            ]
            Html.p [
                prop.className "max-w-xl text-center text-sm text-stone-400 dark:text-stone-500"
                prop.text
                    "No account, no cloud, no plugins. Software changes and companies fold; plain files in a folder you own outlast both."
            ]
        ]
    ]

/// Dismissible strip above the editor for vault-scan warnings and
/// watcher failures.
let private noticeBanner (tone: string) (lines: string list) (onDismiss: unit -> unit) =
    let toneCls =
        match tone with
        | "amber" ->
            "border-amber-300 bg-amber-50 text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200"
        | _ ->
            "border-stone-300 bg-stone-100 text-stone-600 dark:border-stone-600 dark:bg-stone-800 dark:text-stone-300"

    Html.div [
        prop.className (sprintf "flex items-start justify-between gap-3 border-b px-4 py-2 text-xs %s" toneCls)
        prop.children [
            Html.div [
                prop.className "flex flex-col gap-1"
                prop.children (lines |> List.map (fun l -> Html.p [ prop.text l ]))
            ]
            Html.button [
                prop.className "flex-none text-sm opacity-60 hover:opacity-100"
                prop.title "Dismiss"
                prop.onClick (fun _ -> onDismiss ())
                prop.text "×"
            ]
        ]
    ]

/// The dirty-note-changed-on-disk state: both versions are safe, the user
/// picks which one survives. Nothing is written until they choose.
[<ReactComponent>]
let ConflictPane
    (name: string)
    (local: string)
    (disk: string)
    (fontPx: int)
    (onKeepLocal: unit -> unit)
    (onLoadDisk: unit -> unit)
    =
    let showDisk, setShowDisk = React.useState false

    Html.div [
        prop.className "flex h-full min-w-0 flex-1 flex-col"
        prop.children [
            Html.div [
                prop.className
                    "border-b border-amber-300 bg-amber-50 px-6 py-3 dark:border-amber-700 dark:bg-amber-950"
                prop.children [
                    Html.p [
                        prop.className "text-sm font-medium text-amber-800 dark:text-amber-200"
                        prop.text (sprintf "\"%s\" changed on disk while you had unsaved edits." name)
                    ]
                    Html.p [
                        prop.className "mt-1 text-xs text-amber-700 dark:text-amber-300"
                        prop.text
                            "Nothing has been overwritten. Your edits are held here, the other version is on disk. Choose which one to keep — the other is discarded."
                    ]
                    Html.div [
                        prop.className "mt-2 flex flex-wrap items-center gap-2"
                        prop.children [
                            Html.button [
                                prop.className
                                    "rounded bg-emerald-800 px-3 py-1 text-xs text-white hover:bg-emerald-700"
                                prop.onClick (fun _ -> onKeepLocal ())
                                prop.text "Keep my edits (overwrite the file on disk)"
                            ]
                            Html.button [
                                prop.className
                                    "rounded border border-amber-400 px-3 py-1 text-xs text-amber-800 hover:bg-amber-100 dark:border-amber-600 dark:text-amber-200 dark:hover:bg-amber-900"
                                prop.onClick (fun _ -> onLoadDisk ())
                                prop.text "Load the disk version (discard my edits)"
                            ]
                            Html.button [
                                prop.className
                                    "rounded border border-stone-300 px-3 py-1 text-xs text-stone-600 hover:bg-stone-100 dark:border-stone-600 dark:text-stone-300 dark:hover:bg-stone-800"
                                prop.onClick (fun _ -> setShowDisk (not showDisk))
                                prop.text (if showDisk then "Show my edits" else "Show the disk version")
                            ]
                        ]
                    ]
                ]
            ]
            Html.div [
                prop.className "px-6 pt-2 text-[11px] uppercase tracking-wider text-stone-400 dark:text-stone-500"
                prop.text (if showDisk then "Disk version (read-only)" else "My unsaved edits (read-only)")
            ]
            Html.textarea [
                prop.className
                    "flex-1 resize-none bg-transparent px-6 py-4 font-mono leading-relaxed text-stone-800 outline-none dark:text-stone-200"
                prop.style [ style.fontSize (length.px fontPx) ]
                prop.value (if showDisk then disk else local)
                prop.readOnly true
            ]
        ]
    ]

/// The note's file is gone from disk. With unsaved local text the user can
/// recreate the file from it or let it go; a clean note just reports the
/// deletion and offers a deliberate recreate.
let private deletedPane
    (name: string)
    (local: string option)
    (fontPx: int)
    (onKeepLocal: unit -> unit)
    (onDiscard: unit -> unit)
    (onRecreate: string -> unit)
    =
    Html.div [
        prop.className "flex h-full min-w-0 flex-1 flex-col"
        prop.children [
            Html.div [
                prop.className "border-b border-red-200 bg-red-50 px-6 py-3 dark:border-red-900 dark:bg-red-950"
                prop.children [
                    Html.p [
                        prop.className "text-sm font-medium text-red-800 dark:text-red-300"
                        prop.text (sprintf "The file for \"%s\" was deleted outside Plinth." name)
                    ]
                    Html.p [
                        prop.className "mt-1 text-xs text-red-700 dark:text-red-400"
                        prop.text (
                            match local with
                            | Some _ ->
                                "Your unsaved text is safe below and stays in memory until you decide. Autosave is paused so the file is not silently recreated."
                            | None -> "The note was already saved, so nothing of yours was lost. Nothing will be recreated unless you ask."
                        )
                    ]
                    Html.div [
                        prop.className "mt-2 flex flex-wrap items-center gap-2"
                        prop.children [
                            match local with
                            | Some _ ->
                                Html.button [
                                    prop.className
                                        "rounded bg-emerald-800 px-3 py-1 text-xs text-white hover:bg-emerald-700"
                                    prop.onClick (fun _ -> onKeepLocal ())
                                    prop.text "Recreate the file from my text"
                                ]

                                Html.button [
                                    prop.className
                                        "rounded border border-red-300 px-3 py-1 text-xs text-red-700 hover:bg-red-100 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900"
                                    prop.onClick (fun _ -> onDiscard ())
                                    prop.text "Discard my text and move on"
                                ]
                            | None ->
                                Html.button [
                                    prop.className
                                        "rounded bg-emerald-800 px-3 py-1 text-xs text-white hover:bg-emerald-700"
                                    prop.onClick (fun _ -> onRecreate name)
                                    prop.text (sprintf "Recreate \"%s\"" name)
                                ]

                                Html.button [
                                    prop.className
                                        "rounded border border-stone-300 px-3 py-1 text-xs text-stone-600 hover:bg-stone-100 dark:border-stone-600 dark:text-stone-300 dark:hover:bg-stone-800"
                                    prop.onClick (fun _ -> onDiscard ())
                                    prop.text "Move on"
                                ]
                        ]
                    ]
                ]
            ]
            match local with
            | Some text ->
                Html.textarea [
                    prop.className
                        "flex-1 resize-none bg-transparent px-6 py-4 font-mono leading-relaxed text-stone-800 outline-none dark:text-stone-200"
                    prop.style [ style.fontSize (length.px fontPx) ]
                    prop.value text
                    prop.readOnly true
                ]
            | None -> centered [ Html.p [ prop.text "This note no longer has a file on disk." ] ]
        ]
    ]

[<ReactComponent>]
let NoteView () =
    let api = UseNotes.useNotes ()
    let search = UseSearch.useSearch ()
    let settings = useSettings ()
    let showGraph, setShowGraph = React.useState false
    let graphData, setGraphData = React.useState<GraphData option> None
    let showPalette, setShowPalette = React.useState false
    let showRename, setShowRename = React.useState false
    let showNewNote, setShowNewNote = React.useState false
    let showShortcuts, setShowShortcuts = React.useState false
    let justSaved, setJustSaved = React.useState false

    let currentName =
        match api.Current with
        | EditingNote(n, _)
        | LoadingNote n
        | ConflictNote(n, _, _)
        | DeletedNote(n, _) -> Some n
        | _ -> None

    // Case-insensitive existence check drives broken-link styling in preview.
    let noteSet =
        api.Notes
        |> Array.map (fun n -> n.Name.ToLowerInvariant())
        |> Set.ofArray

    let noteExists (name: string) =
        noteSet.Contains (name.Trim().ToLowerInvariant())

    // Fetch a fresh snapshot each time the Firmament opens, so new
    // notes and links written since the last look are already in the sky.
    let openGraph () =
        promise {
            try
                let! g = Tauri.getGraph ()
                setGraphData (Some g)
                setShowGraph true
            with _ -> ()
        }
        |> Promise.start

    let hasVault = api.Current <> NoVault

    let isEditing =
        match api.Current with
        | EditingNote _ -> true
        | _ -> false

    // Ctrl/Cmd+D opens (creating if needed) today's daily note;
    // Ctrl/Cmd+N asks for a name and opens that.
    let keyboardEffect () : unit -> unit =
        let handler (e: Browser.Types.Event) =
            let ke = e :?> Browser.Types.KeyboardEvent

            if (ke.ctrlKey || ke.metaKey) && ke.key.ToLowerInvariant() = "d" then
                ke.preventDefault ()
                api.OpenToday ()
            elif (ke.ctrlKey || ke.metaKey) && ke.key.ToLowerInvariant() = "n" then
                ke.preventDefault ()
                setShowNewNote true
            elif (ke.ctrlKey || ke.metaKey) && ke.key.ToLowerInvariant() = "s" then
                // Stop the browser's Save Page dialog in dev mode too.
                ke.preventDefault ()
                api.SaveNow ()
                setJustSaved true

        window.addEventListener ("keydown", handler)
        fun () -> window.removeEventListener ("keydown", handler)

    React.useEffect (keyboardEffect, [||])

    // The "saved ✓" acknowledgement fades on its own.
    let savedFlashEffect () : unit -> unit =
        if justSaved then
            let timer = Fable.Core.JS.setTimeout (fun () -> setJustSaved false) 1400
            fun () -> Fable.Core.JS.clearTimeout timer
        else
            ignore

    React.useEffect (savedFlashEffect, [| box justSaved |])

    // Ctrl/Cmd+K palette and Ctrl/Cmd+G Firmament. Re-bound whenever
    // the toggles change so the handler never closes over stale state.
    let overlayKeysEffect () : unit -> unit =
        let handler (e: Browser.Types.Event) =
            let ke = e :?> Browser.Types.KeyboardEvent
            let key = ke.key.ToLowerInvariant()

            if (ke.ctrlKey || ke.metaKey) && (key = "k" || key = "p") then
                ke.preventDefault ()

                if hasVault then
                    setShowPalette (not showPalette)
            elif (ke.ctrlKey || ke.metaKey) && key = "g" then
                ke.preventDefault ()

                if hasVault then
                    if showGraph then setShowGraph false else openGraph ()

        window.addEventListener ("keydown", handler)
        fun () -> window.removeEventListener ("keydown", handler)

    React.useEffect (overlayKeysEffect, [| box showPalette; box showGraph; box hasVault |])

    let paletteActions: Palette.PaletteAction list =
        [ yield
              { Palette.PaletteAction.Label = "New note…"
                Palette.PaletteAction.Hint = "Ctrl+N"
                Palette.PaletteAction.Run = fun () -> setShowNewNote true }
          yield
              { Label = "Open today's daily note"
                Hint = "Ctrl+D"
                Run = api.OpenToday }
          yield
              { Label = "Open the Firmament"
                Hint = "Ctrl+G"
                Run = openGraph }
          if isEditing then
              yield
                  { Label = "Rename current note…"
                    Hint = "note"
                    Run = fun () -> setShowRename true }
          yield
              { Label =
                  (if settings.Theme = Dark then
                       "Switch to light theme"
                   else
                       "Switch to dark theme")
                Hint = "theme"
                Run =
                  fun () ->
                      settings.SetTheme (
                          if settings.Theme = Dark then Light else Dark
                      ) }
          yield
              { Label = "Keyboard shortcuts…"
                Hint = "help"
                Run = fun () -> setShowShortcuts true }
          yield
              { Label = "Export vault as .zip…"
                Hint = "vault"
                Run = api.ExportVault }
          yield
              { Label = "Change vault folder…"
                Hint = "vault"
                Run = api.PickVault } ]

    Html.div [
        prop.className (
            (if settings.Theme = Dark then "dark " else "")
            + "relative flex h-screen bg-stone-50 font-sans text-stone-800 dark:bg-stone-900 dark:text-stone-200"
        )
        prop.children [
            match api.Current with
            | NoVault -> welcome api.WelcomeInfo api.StartNewVault api.PickVault api.OpenSampleVault
            | _ ->
                Html.aside [
                    prop.className
                        "flex w-64 flex-none flex-col border-r border-stone-200 bg-stone-100/60 dark:border-stone-700 dark:bg-stone-800/60"
                    prop.children [
                        Html.div [
                            prop.className "flex items-center justify-between px-4 pt-4"
                            prop.children [
                                Html.span [
                                    prop.className
                                        "font-serif text-xl font-bold tracking-tight text-emerald-900 dark:text-emerald-300"
                                    prop.text "Plinth"
                                ]
                                Settings.SettingsMenu
                                    settings
                                    api.Vault
                                    api.PickVault
                                    api.ExportVault
                                    (fun () -> setShowShortcuts true)
                            ]
                        ]
                        Search.SearchBox search api.OpenNote
                        Sidebar.Sidebar api (fun () -> setShowNewNote true) openGraph
                    ]
                ]

                Html.div [
                    prop.className "flex h-full min-w-0 flex-1 flex-col"
                    prop.children [
                        if api.Warnings.Length > 0 then
                            noticeBanner "amber" (api.Warnings |> Array.toList) api.DismissWarnings

                        // Deliberately quiet: a notice is Plinth explaining
                        // itself, not reporting a fault, and the first one
                        // most people meet is "your subfolders were left
                        // alone" on the very first folder they open.
                        if api.Notices.Length > 0 then
                            noticeBanner "stone" (api.Notices |> Array.toList) api.DismissNotices

                        match api.WatcherError with
                        | Some err ->
                            noticeBanner
                                "stone"
                                [ sprintf
                                      "Plinth can't watch this folder for outside changes (%s). Edits made in other apps won't show up until the vault is reopened."
                                      err ]
                                api.DismissWatcherError
                        | None -> Html.none

                        match api.Current with
                        | NoVault -> Html.none
                        | VaultLoading -> centered [ Html.p [ prop.text "Indexing vault…" ] ]
                        | LoadingNote n -> centered [ Html.p [ prop.text (sprintf "Opening %s…" n) ] ]
                        | NoteError msg ->
                            centered [
                                Html.div [
                                    prop.className
                                        "max-w-md rounded-lg border border-red-200 bg-red-50 p-6 text-center dark:border-red-900 dark:bg-red-950"
                                    prop.children [
                                        Html.p [
                                            prop.className "font-medium text-red-800 dark:text-red-300"
                                            prop.text "Something went wrong"
                                        ]
                                        Html.p [
                                            prop.className "mt-2 text-sm text-red-600 dark:text-red-400"
                                            prop.text msg
                                        ]
                                        Html.button [
                                            prop.className
                                                "mt-4 rounded bg-emerald-800 px-4 py-1.5 text-sm text-white hover:bg-emerald-700"
                                            prop.onClick (fun _ -> api.OpenToday ())
                                            prop.text "Back to today's note"
                                        ]
                                    ]
                                ]
                            ]
                        | ConflictNote(name, local, disk) ->
                            ConflictPane name local disk settings.EditorPx api.KeepLocalVersion api.LoadDiskVersion
                        | DeletedNote(name, local) ->
                            deletedPane name local settings.EditorPx api.KeepLocalVersion api.DiscardLocal api.OpenNote
                        | EditingNote(name, content) ->
                            Editor.Editor
                                { Name = name
                                  Content = content
                                  Dirty = api.Dirty
                                  JustSaved = justSaved
                                  FontPx = settings.EditorPx
                                  NoteNames = api.Notes |> Array.map (fun n -> n.Name)
                                  NoteExists = noteExists
                                  OnChange = api.UpdateContent
                                  OnLink = api.OpenNote
                                  OnTag = api.FilterByTag
                                  OnExternal = fun url -> Tauri.openExternal url |> Promise.start
                                  OnDelete = fun () -> api.DeleteNote name
                                  OnRenameClick = fun () -> setShowRename true }
                    ]
                ]

                Backlinks.Backlinks currentName api.Backlinks api.OpenNote

                if showGraph then
                    match graphData with
                    | Some g ->
                        Graph.Firmament
                            { Data = g
                              Current = currentName
                              Dark = settings.Theme = Dark
                              OnOpen =
                                fun name ->
                                    setShowGraph false
                                    api.OpenNote name
                              OnClose = fun () -> setShowGraph false }
                    | None -> Html.none

                if showPalette then
                    Palette.Palette
                        { Notes = api.Notes
                          Recents = api.Recents
                          Actions = paletteActions
                          OnOpen = api.OpenNote
                          OnClose = fun () -> setShowPalette false }

                if showNewNote then
                    Editor.NewNoteDialog noteExists api.OpenNote (fun () -> setShowNewNote false)

                if showShortcuts then
                    Settings.ShortcutsDialog (fun () -> setShowShortcuts false)

                match api.Current with
                | EditingNote(name, _) when showRename ->
                    Editor.RenameDialog name api.RenameNote (fun () -> setShowRename false)
                | _ -> Html.none
        ]
    ]
