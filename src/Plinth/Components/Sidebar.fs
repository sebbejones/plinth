/// Left rail: recent notes, note list (optionally filtered by tag),
/// and the tag explorer.
module Plinth.Components.Sidebar

open Feliz
open Plinth.Types
open Plinth.Hooks.UseNotes

let private sectionTitle (text: string) =
    Html.h2 [
        prop.className
            "px-4 pt-4 pb-1 text-[11px] font-semibold uppercase tracking-wider text-stone-400 dark:text-stone-500"
        prop.text text
    ]

let private mutedNote (text: string) =
    Html.p [ prop.className "px-4 text-xs text-stone-400 dark:text-stone-500"; prop.text text ]

[<ReactComponent>]
let Sidebar (api: NotesApi) (onNewNote: unit -> unit) (onGraph: unit -> unit) =
    let currentName =
        match api.Current with
        | EditingNote(n, _)
        | LoadingNote n -> Some n
        | _ -> None

    let noteButton (keyPrefix: string) (name: string) =
        Html.button [
            prop.key (keyPrefix + name)
            prop.className (
                "block w-full truncate px-4 py-1.5 text-left text-sm hover:bg-stone-200 dark:hover:bg-stone-700 "
                + (if currentName = Some name then
                       "bg-stone-200 font-medium text-emerald-900 dark:bg-stone-700 dark:text-emerald-300"
                   else
                       "text-stone-700 dark:text-stone-300")
            )
            prop.onClick (fun _ -> api.OpenNote name)
            prop.text name
        ]

    Html.div [
        prop.className "flex h-full flex-col overflow-y-auto pb-6"
        prop.children [
            Html.div [
                prop.className "flex items-center gap-2 px-3 pt-3"
                prop.children [
                    Html.button [
                        prop.className
                            "flex-1 rounded bg-emerald-800 px-3 py-1.5 text-sm text-white hover:bg-emerald-700"
                        prop.title "Start a new note (Ctrl+N)"
                        prop.onClick (fun _ -> onNewNote ())
                        prop.text "+ New note"
                    ]
                    Html.button [
                        prop.className
                            "rounded border border-stone-300 px-2.5 py-1.5 text-sm text-emerald-800 hover:bg-stone-200 dark:border-stone-600 dark:text-emerald-300 dark:hover:bg-stone-700"
                        prop.title "Open today's daily note (Ctrl+D)"
                        prop.onClick (fun _ -> api.OpenToday ())
                        prop.text "Today"
                    ]
                    Html.button [
                        prop.className
                            "rounded border border-stone-300 px-2.5 py-1.5 text-sm text-emerald-800 hover:bg-stone-200 dark:border-stone-600 dark:text-emerald-300 dark:hover:bg-stone-700"
                        prop.title "Open the Firmament (Ctrl+G)"
                        prop.onClick (fun _ -> onGraph ())
                        prop.text "✦"
                    ]
                ]
            ]
            // Recent notes (skip when there's nothing beyond the current note).
            if api.Recents.Length > 1 then
                Html.div [
                    prop.children [
                        sectionTitle "Recent"
                        yield! api.Recents |> Array.toList |> List.map (noteButton "recent-")
                    ]
                ]
            match api.TagFilter with
            | Some(tag, names) ->
                Html.div [
                    prop.children [
                        Html.div [
                            prop.className "flex items-baseline justify-between pr-3"
                            prop.children [
                                sectionTitle (sprintf "#%s (%i)" tag names.Length)
                                Html.button [
                                    prop.className
                                        "text-xs text-stone-400 hover:text-stone-600 dark:text-stone-500 dark:hover:text-stone-300"
                                    prop.onClick (fun _ -> api.ClearTagFilter ())
                                    prop.text "clear"
                                ]
                            ]
                        ]
                        yield! names |> Array.toList |> List.map (noteButton "tag-")
                    ]
                ]
            | None ->
                Html.div [
                    prop.children [
                        sectionTitle (sprintf "Notes (%i)" api.Notes.Length)
                        if api.Notes.Length = 0 then
                            mutedNote "No notes yet — hit New note or Today to start."
                        yield! api.Notes |> Array.toList |> List.map (fun n -> noteButton "note-" n.Name)
                    ]
                ]
            sectionTitle "Tags"
            if api.Tags.Length = 0 then
                mutedNote "No tags yet — type #something in a note."
            else
                Html.div [
                    prop.className "flex flex-wrap gap-1 px-4"
                    prop.children (
                        api.Tags
                        |> Array.toList
                        |> List.map (fun t ->
                            Html.button [
                                prop.key t.Tag
                                prop.className
                                    "rounded-full bg-stone-200 px-2 py-0.5 text-xs text-stone-600 hover:bg-amber-100 hover:text-amber-800 dark:bg-stone-700 dark:text-stone-300 dark:hover:bg-amber-900 dark:hover:text-amber-200"
                                prop.onClick (fun _ -> api.FilterByTag t.Tag)
                                prop.text (sprintf "#%s (%i)" t.Tag t.Count)
                            ])
                    )
                ]
        ]
    ]
