/// Plain-textarea editor with a Preview toggle. No rich text, no WYSIWYG.
module Plinth.Components.Editor

open Fable.Core
open Feliz
open Plinth.Utils

type EditorProps =
    { Name: string
      Content: string
      Dirty: bool
      FontPx: int
      NoteExists: string -> bool
      OnChange: string -> unit
      OnLink: string -> unit
      OnTag: string -> unit
      OnDelete: unit -> unit
      OnRenameClick: unit -> unit }

/// Small modal for renaming the current note. Validation and link
/// rewriting happen in the backend; this just carries the name across
/// and shows any error it gets back.
[<ReactComponent>]
let RenameDialog (name: string) (onRename: string -> JS.Promise<string option>) (onClose: unit -> unit) =
    let value, setValue = React.useState name
    let error, setError = React.useState<string option> None
    let busy, setBusy = React.useState false

    let submit () =
        let trimmed = value.Trim()

        if trimmed = "" then
            setError (Some "Enter a name.")
        elif trimmed = name then
            onClose ()
        elif not busy then
            setBusy true

            promise {
                let! err = onRename trimmed

                match err with
                | None -> onClose ()
                | Some e ->
                    setBusy false
                    setError (Some e)
            }
            |> Promise.start

    Html.div [
        prop.className "absolute inset-0 z-50 bg-black/30"
        prop.onClick (fun _ -> onClose ())
        prop.children [
            Html.div [
                prop.className
                    "mx-auto mt-[20vh] w-full max-w-md rounded-xl border border-stone-200 bg-white p-5 shadow-2xl dark:border-stone-600 dark:bg-stone-800"
                prop.onClick (fun e -> e.stopPropagation ())
                prop.children [
                    Html.h2 [
                        prop.className "font-serif text-lg font-semibold text-stone-800 dark:text-stone-100"
                        prop.text "Rename note"
                    ]
                    Html.p [
                        prop.className "mt-1 text-xs text-stone-500 dark:text-stone-400"
                        prop.text "The file is renamed and every [[link]] to it in the vault is updated."
                    ]
                    Html.input [
                        prop.className
                            "mt-3 w-full rounded border border-stone-300 bg-white px-3 py-1.5 text-sm text-stone-800 outline-none focus:border-emerald-600 dark:border-stone-600 dark:bg-stone-900 dark:text-stone-200"
                        prop.autoFocus true
                        prop.value value
                        prop.onChange (fun (v: string) ->
                            setValue v
                            setError None)
                        prop.onKeyDown (fun e ->
                            match e.key with
                            | "Enter" ->
                                e.preventDefault ()
                                submit ()
                            | "Escape" ->
                                e.preventDefault ()
                                e.stopPropagation ()
                                onClose ()
                            | _ -> ())
                    ]
                    match error with
                    | Some msg ->
                        Html.p [
                            prop.className "mt-2 text-xs text-red-600 dark:text-red-400"
                            prop.text msg
                        ]
                    | None -> Html.none
                    Html.div [
                        prop.className "mt-4 flex justify-end gap-2"
                        prop.children [
                            Html.button [
                                prop.className
                                    "rounded border border-stone-300 px-3 py-1 text-sm text-stone-600 hover:bg-stone-100 dark:border-stone-600 dark:text-stone-300 dark:hover:bg-stone-700"
                                prop.onClick (fun _ -> onClose ())
                                prop.text "Cancel"
                            ]
                            Html.button [
                                prop.className
                                    "rounded bg-emerald-800 px-3 py-1 text-sm text-white hover:bg-emerald-700 disabled:opacity-50"
                                prop.disabled busy
                                prop.onClick (fun _ -> submit ())
                                prop.text (if busy then "Renaming…" else "Rename")
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ]

[<ReactComponent>]
let Editor (props: EditorProps) =
    let preview, setPreview = React.useState false

    Html.div [
        prop.className "flex h-full min-w-0 flex-1 flex-col"
        prop.children [
            Html.div [
                prop.className
                    "flex items-center justify-between border-b border-stone-200 px-6 py-3 dark:border-stone-700"
                prop.children [
                    Html.div [
                        prop.className "flex items-baseline gap-3"
                        prop.children [
                            Html.h1 [
                                prop.className "font-serif text-lg font-semibold text-stone-800 dark:text-stone-100"
                                prop.text props.Name
                            ]
                            Html.span [
                                prop.className (
                                    if props.Dirty then "text-xs text-amber-600 dark:text-amber-400"
                                    else "text-xs text-stone-400 dark:text-stone-500"
                                )
                                prop.text (if props.Dirty then "unsaved" else "saved")
                            ]
                        ]
                    ]
                    Html.div [
                        prop.className "flex items-center gap-2"
                        prop.children [
                            Html.button [
                                prop.className
                                    "rounded border border-stone-300 px-3 py-1 text-sm text-stone-600 hover:bg-stone-100 dark:border-stone-600 dark:text-stone-300 dark:hover:bg-stone-800"
                                prop.title "Rename this note and update links to it"
                                prop.onClick (fun _ -> props.OnRenameClick ())
                                prop.text "Rename"
                            ]
                            Html.button [
                                prop.className
                                    "rounded border border-stone-300 px-3 py-1 text-sm text-stone-600 hover:bg-stone-100 dark:border-stone-600 dark:text-stone-300 dark:hover:bg-stone-800"
                                prop.onClick (fun _ -> setPreview (not preview))
                                prop.text (if preview then "Edit" else "Preview")
                            ]
                            Html.button [
                                prop.className
                                    "rounded border border-red-200 px-3 py-1 text-sm text-red-600 hover:bg-red-50 dark:border-red-900 dark:text-red-400 dark:hover:bg-red-950"
                                prop.title "Delete this note"
                                prop.onClick (fun _ -> props.OnDelete ())
                                prop.text "Delete"
                            ]
                        ]
                    ]
                ]
            ]
            if preview then
                Html.div [
                    prop.className "flex-1 overflow-y-auto px-6 py-4"
                    prop.style [ style.fontSize (length.px props.FontPx) ]
                    prop.children [ Markdown.render props.Content props.NoteExists props.OnLink props.OnTag ]
                ]
            else
                Html.textarea [
                    prop.className
                        "flex-1 resize-none bg-transparent px-6 py-4 font-mono leading-relaxed text-stone-800 outline-none dark:text-stone-200"
                    prop.style [ style.fontSize (length.px props.FontPx) ]
                    prop.value props.Content
                    prop.custom ("spellCheck", false)
                    prop.placeholder "Type your thoughts. Link with [[Note Name]], tag with #tag."
                    prop.onChange (fun (v: string) -> props.OnChange v)
                ]
        ]
    ]
