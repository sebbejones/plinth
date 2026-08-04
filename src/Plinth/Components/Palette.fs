/// Ctrl+K command palette: jump to any note by fuzzy name match,
/// create one that doesn't exist yet, or run an app action — all
/// without touching the mouse. Matching runs over the in-memory note
/// list, so results are instant with no round trip to the backend.
module Plinth.Components.Palette

open Feliz
open Plinth.Types
open Plinth.Utils

type PaletteAction =
    { Label: string
      Hint: string
      Run: unit -> unit }

type PaletteProps =
    { Notes: NoteMeta[]
      Recents: string[]
      Actions: PaletteAction list
      OnOpen: string -> unit
      OnClose: unit -> unit }

type private Item =
    | NoteItem of name: string * hint: string
    | CreateItem of name: string
    | ActionItem of PaletteAction

[<ReactComponent>]
let Palette (props: PaletteProps) =
    let query, setQuery = React.useState ""
    let selected, setSelected = React.useState 0

    let q = query.Trim()

    let items =
        if q = "" then
            [ yield! props.Recents |> Seq.truncate 6 |> Seq.map (fun n -> NoteItem(n, "recent"))
              yield! props.Actions |> List.map ActionItem ]
        else
            let noteHits =
                props.Notes
                |> Array.map (fun n -> n.Name)
                |> Fuzzy.rank 8 q
                |> Array.toList
                |> List.map (fun name -> NoteItem(name, "note"))

            let actionHits =
                props.Actions
                |> List.filter (fun a -> a.Label.ToLowerInvariant().Contains(q.ToLowerInvariant()))
                |> List.map ActionItem

            let exactExists =
                props.Notes
                |> Array.exists (fun n -> System.String.Equals(n.Name, q, System.StringComparison.OrdinalIgnoreCase))

            let creatable =
                not exactExists
                && not (q.Contains "/" || q.Contains "\\" || q.Contains ":" || q.Contains "..")

            [ yield! noteHits
              yield! actionHits
              if creatable then
                  yield CreateItem q ]

    let run (item: Item) =
        match item with
        | NoteItem(name, _) ->
            props.OnOpen name
            props.OnClose ()
        | CreateItem name ->
            props.OnOpen name
            props.OnClose ()
        | ActionItem a ->
            props.OnClose ()
            a.Run ()

    let itemCount = List.length items
    let clamped = if itemCount = 0 then 0 else min selected (itemCount - 1)

    let onKeyDown (e: Browser.Types.KeyboardEvent) =
        match e.key with
        | "Escape" ->
            e.preventDefault ()
            // Keep it from reaching the constellation's Esc handler below.
            e.stopPropagation ()
            props.OnClose ()
        | "ArrowDown" ->
            e.preventDefault ()
            setSelected (min (clamped + 1) (max 0 (itemCount - 1)))
        | "ArrowUp" ->
            e.preventDefault ()
            setSelected (max (clamped - 1) 0)
        | "Enter" ->
            e.preventDefault ()

            match List.tryItem clamped items with
            | Some item -> run item
            | None -> ()
        | _ -> ()

    let renderItem (i: int) (item: Item) =
        let label, hint =
            match item with
            | NoteItem(name, hint) -> name, hint
            | CreateItem name -> sprintf "Create \"%s\"" name, "new note"
            | ActionItem a -> a.Label, a.Hint

        Html.button [
            prop.key (string i + label)
            prop.className (
                "flex w-full items-baseline justify-between gap-3 px-4 py-2 text-left text-sm "
                + (if i = clamped then
                       "bg-emerald-800 text-white"
                   else
                       "text-stone-700 hover:bg-stone-100 dark:text-stone-300 dark:hover:bg-stone-700")
            )
            prop.onMouseEnter (fun _ -> setSelected i)
            prop.onClick (fun _ -> run item)
            prop.children [
                Html.span [ prop.className "truncate"; prop.text label ]
                Html.span [
                    prop.className (
                        "flex-none text-[10px] uppercase tracking-wider "
                        + (if i = clamped then "text-emerald-200" else "text-stone-400 dark:text-stone-500")
                    )
                    prop.text hint
                ]
            ]
        ]

    Html.div [
        prop.className "absolute inset-0 z-50 bg-black/30"
        prop.onClick (fun _ -> props.OnClose ())
        prop.children [
            Html.div [
                prop.className
                    "mx-auto mt-[15vh] w-full max-w-lg overflow-hidden rounded-xl border border-stone-200 bg-white shadow-2xl dark:border-stone-600 dark:bg-stone-800"
                prop.onClick (fun e -> e.stopPropagation ())
                prop.children [
                    Html.input [
                        prop.className
                            "w-full border-b border-stone-200 bg-transparent px-4 py-3 text-sm text-stone-800 outline-none dark:border-stone-600 dark:text-stone-200"
                        prop.placeholder "Jump to a note, create one, or run a command…"
                        prop.autoFocus true
                        prop.value query
                        prop.onChange (fun (v: string) ->
                            setQuery v
                            setSelected 0)
                        prop.onKeyDown onKeyDown
                    ]
                    if itemCount = 0 then
                        Html.p [
                            prop.className "px-4 py-3 text-sm text-stone-400 dark:text-stone-500"
                            prop.text "Nothing matches."
                        ]
                    else
                        Html.div [
                            prop.className "max-h-80 overflow-y-auto py-1"
                            prop.children (items |> List.mapi renderItem)
                        ]
                ]
            ]
        ]
    ]
