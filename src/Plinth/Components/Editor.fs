/// Plain-textarea editor with a Preview toggle. No rich text, no WYSIWYG.
module Plinth.Components.Editor

open Fable.Core
open Fable.Core.JsInterop
open Feliz
open Plinth.Utils

type EditorProps =
    { Name: string
      Content: string
      Dirty: bool
      /// Briefly true after an explicit Ctrl+S, so the save can be seen.
      JustSaved: bool
      FontPx: int
      /// Every note name in the vault, for the [[link]] menu.
      NoteNames: string[]
      NoteExists: string -> bool
      OnChange: string -> unit
      OnLink: string -> unit
      OnTag: string -> unit
      OnDelete: unit -> unit
      OnRenameClick: unit -> unit }

/// CSS the caret mirror has to match for its text to wrap identically.
let private mirroredCss =
    [ "font-family"
      "font-size"
      "font-weight"
      "font-style"
      "line-height"
      "letter-spacing"
      "text-indent"
      "padding-top"
      "padding-right"
      "padding-bottom"
      "padding-left"
      "border-top-width"
      "border-left-width" ]

/// Where the caret sits, in pixels inside the textarea, plus the height of
/// its line. A textarea exposes no caret geometry and this one soft-wraps,
/// so the only reliable way is to lay the text out again in a hidden div
/// with the same metrics and measure where it ends up.
let private caretXY (ta: Browser.Types.HTMLTextAreaElement) (caret: int) =
    let doc = Browser.Dom.document
    // getComputedStyle and the offset* properties aren't in Fable's typed
    // DOM bindings, so these few reads go through plain JS interop.
    let cs: obj = Browser.Dom.window?getComputedStyle (ta)

    let copied =
        mirroredCss
        |> List.map (fun p -> p + ":" + (cs?getPropertyValue (p): string))
        |> String.concat ";"

    let mirror = doc.createElement "div"

    mirror.setAttribute (
        "style",
        copied
        + sprintf
            ";position:absolute;visibility:hidden;white-space:pre-wrap;word-wrap:break-word;box-sizing:border-box;top:0;left:-9999px;width:%ipx"
            (int ta.clientWidth)
    )

    mirror.textContent <- ta.value.Substring(0, caret)

    // A zero-width space so the marker still occupies its line.
    let marker = doc.createElement "span"
    marker.textContent <- "​"
    mirror.appendChild marker |> ignore
    doc.body.appendChild mirror |> ignore

    let x: float = marker?offsetLeft
    let y: float = marker?offsetTop
    let h: float = marker?offsetHeight
    doc.body.removeChild mirror |> ignore

    x, y - ta.scrollTop, h

/// If the caret sits inside an unclosed `[[`, return where the brackets
/// start and what has been typed since. A `]` or a line break in between
/// means the link is already finished (or was never one).
let private linkQuery (text: string) (caret: int) =
    let before = text.Substring(0, caret)
    let start = before.LastIndexOf "[["

    if start < 0 then
        None
    else
        let typed = before.Substring(start + 2)

        if typed.Contains "]" || typed.Contains "\n" || typed.Length > 80 then
            None
        else
            Some(start, typed)

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

/// Small modal for starting a new note. Creation itself is just "open a
/// note that isn't there yet", so this only has to collect a usable name.
/// A name that already exists isn't an error — the button turns into Open.
[<ReactComponent>]
let NewNoteDialog (exists: string -> bool) (onOpen: string -> unit) (onClose: unit -> unit) =
    let value, setValue = React.useState ""

    let trimmed = value.Trim()

    // Same characters the command palette refuses: the note name is a
    // filename in the vault root, nothing more.
    let illegal =
        trimmed.Contains "/"
        || trimmed.Contains "\\"
        || trimmed.Contains ":"
        || trimmed.Contains ".."

    let alreadyExists = trimmed <> "" && exists trimmed
    let canSubmit = trimmed <> "" && not illegal

    let submit () =
        if canSubmit then
            onOpen trimmed
            onClose ()

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
                        prop.text "New note"
                    ]
                    Html.p [
                        prop.className "mt-1 text-xs text-stone-500 dark:text-stone-400"
                        prop.text "The name becomes the filename, and [[the name]] links to it."
                    ]
                    Html.input [
                        prop.className
                            "mt-3 w-full rounded border border-stone-300 bg-white px-3 py-1.5 text-sm text-stone-800 outline-none focus:border-emerald-600 dark:border-stone-600 dark:bg-stone-900 dark:text-stone-200"
                        prop.autoFocus true
                        prop.placeholder "Note name"
                        prop.value value
                        prop.onChange (fun (v: string) -> setValue v)
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
                    if illegal then
                        Html.p [
                            prop.className "mt-2 text-xs text-red-600 dark:text-red-400"
                            prop.text "A note name can't contain / \\ : or .."
                        ]
                    elif alreadyExists then
                        Html.p [
                            prop.className "mt-2 text-xs text-stone-500 dark:text-stone-400"
                            prop.text "That note already exists — this will open it."
                        ]
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
                                prop.disabled (not canSubmit)
                                prop.onClick (fun _ -> submit ())
                                prop.text (if alreadyExists then "Open" else "Create")
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
    let taRef = React.useElementRef ()
    // Set while an unclosed [[ is under the caret: bracket offset and pixels.
    let suggest, setSuggest = React.useState<(int * float * float) option> None
    let query, setQuery = React.useState ""
    let sel, setSel = React.useState 0
    // Caret position to restore after an insertion re-renders the textarea.
    let pendingCaret = React.useRef<int option> None

    let textarea () =
        taRef.current |> Option.map (fun el -> unbox<Browser.Types.HTMLTextAreaElement> el)

    let matches =
        match suggest with
        | Some _ -> props.NoteNames |> Fuzzy.rank 6 (query.Trim())
        | None -> [||]

    /// Recompute the menu from wherever the caret now is.
    let refresh () =
        match textarea () with
        | Some ta ->
            let caret = int ta.selectionStart

            match linkQuery ta.value caret with
            | Some(start, typed) ->
                let x, y, h = caretXY ta caret
                setSuggest (Some(start, x, y + h))
                setQuery typed
                setSel 0
            | None -> setSuggest None
        | None -> ()

    /// Finish the link: replace what was typed with the chosen name, close
    /// the brackets, and leave the caret after them so typing carries on.
    let accept (name: string) =
        match textarea (), suggest with
        | Some ta, Some(start, _, _) ->
            let caret = int ta.selectionStart
            let text = ta.value
            let head = text.Substring(0, start + 2)
            let tail = text.Substring(caret)
            pendingCaret.current <- Some(head.Length + name.Length + 2)
            setSuggest None
            props.OnChange (head + name + "]]" + tail)
        | _ -> ()

    // Restoring the caret has to wait for React to paint the new text.
    React.useEffect (fun () ->
        match pendingCaret.current, textarea () with
        | Some pos, Some ta ->
            pendingCaret.current <- None
            ta.focus ()
            ta.setSelectionRange (pos, pos)
        | _ -> ())

    let onKeyDown (e: Browser.Types.KeyboardEvent) =
        if suggest.IsSome && matches.Length > 0 then
            let clamped = min sel (matches.Length - 1)

            // "Down"/"Up"/"Esc" are the pre-standard names; some webviews
            // still report them, and accepting both costs nothing.
            match e.key with
            | "ArrowDown"
            | "Down" ->
                e.preventDefault ()
                setSel ((clamped + 1) % matches.Length)
            | "ArrowUp"
            | "Up" ->
                e.preventDefault ()
                setSel ((clamped - 1 + matches.Length) % matches.Length)
            | "Enter"
            | "Tab" ->
                e.preventDefault ()
                accept matches.[clamped]
            | "Escape"
            | "Esc" ->
                e.preventDefault ()
                // Don't let it reach the overlay handlers behind the editor.
                e.stopPropagation ()
                setSuggest None
            | _ -> ()

    // Arrowing through the menu moves the caret too; that must not be
    // mistaken for the user typing and reset the highlighted row.
    let onKeyUp (e: Browser.Types.KeyboardEvent) =
        let navigating =
            match e.key with
            | "ArrowDown"
            | "Down"
            | "ArrowUp"
            | "Up"
            | "Enter"
            | "Tab"
            | "Escape"
            | "Esc" -> true
            | _ -> false

        if not (suggest.IsSome && navigating) then
            refresh ()

    let linkMenu (x: float) (y: float) =
        // Keep the menu inside the editor even when the caret is far right.
        let left =
            match textarea () with
            | Some ta -> max 0.0 (min x (ta.clientWidth - 240.0))
            | None -> x

        Html.div [
            prop.className
                "absolute z-30 w-60 overflow-hidden rounded-lg border border-stone-200 bg-white py-1 shadow-xl dark:border-stone-600 dark:bg-stone-800"
            prop.style [ style.left (length.px left); style.top (length.px y) ]
            prop.children (
                matches
                |> Array.toList
                |> List.mapi (fun i name ->
                    Html.button [
                        prop.key name
                        prop.className (
                            "block w-full truncate px-3 py-1.5 text-left text-sm "
                            + (if i = min sel (matches.Length - 1) then
                                   "bg-emerald-800 text-white"
                               else
                                   "text-stone-700 hover:bg-stone-100 dark:text-stone-300 dark:hover:bg-stone-700")
                        )
                        // mousedown, not click: clicking must not blur the
                        // textarea first or the caret position is lost.
                        prop.onMouseDown (fun e ->
                            e.preventDefault ()
                            accept name)
                        prop.text name
                    ])
            )
        ]

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
                                    elif props.JustSaved then "text-xs text-emerald-700 dark:text-emerald-400"
                                    else "text-xs text-stone-400 dark:text-stone-500"
                                )
                                prop.text (
                                    if props.Dirty then "unsaved"
                                    elif props.JustSaved then "saved ✓"
                                    else "saved"
                                )
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
                Html.div [
                    prop.className "relative flex min-h-0 flex-1"
                    prop.children [
                        Html.textarea [
                            prop.ref taRef
                            prop.className
                                "h-full w-full resize-none bg-transparent px-6 py-4 font-mono leading-relaxed text-stone-800 outline-none dark:text-stone-200"
                            prop.style [ style.fontSize (length.px props.FontPx) ]
                            prop.value props.Content
                            prop.custom ("spellCheck", false)
                            prop.placeholder "Type your thoughts. Link with [[Note Name]], tag with #tag."
                            prop.onChange (fun (v: string) ->
                                props.OnChange v
                                refresh ())
                            prop.onKeyDown onKeyDown
                            // The caret also moves by arrow key and by click.
                            prop.onKeyUp onKeyUp
                            prop.onClick (fun _ -> refresh ())
                        ]
                        match suggest with
                        | Some(_, x, y) when matches.Length > 0 -> linkMenu x y
                        | _ -> Html.none
                    ]
                ]
        ]
    ]
