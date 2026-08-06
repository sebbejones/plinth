/// Markdown preview. The parsing and sanitising live in `src/markdown.js`
/// (markdown-it + DOMPurify, both bundled); this is the React surface.
///
/// The renderer returns an HTML string rather than React elements, so
/// [[wiki links]], #tags and web links carry `data-` attributes and one
/// delegated click handler on the container dispatches them. That is why
/// the preview is a single `dangerouslySetInnerHTML` div: every path into
/// it has been through DOMPurify.
module Plinth.Utils.Markdown

open Fable.Core
open Browser.Types
open Feliz

[<Import("renderNote", "../../markdown.js")>]
let private renderNote (content: string) (noteExists: string -> bool) : string = jsNative

/// Render a note body as a preview.
///
/// `onExternal` receives an http(s)/mailto URL: it must hand it to the OS
/// browser. Letting the WebView follow it would replace the app with the
/// page and leave no way back.
let render
    (content: string)
    (noteExists: string -> bool)
    (onLink: string -> unit)
    (onTag: string -> unit)
    (onExternal: string -> unit)
    =
    let onClick (e: MouseEvent) =
        let target = e.target :?> Element

        match target.closest "[data-wikilink],[data-tag],[data-external]" with
        | None -> ()
        | Some hit ->
            e.preventDefault ()

            let attr (name: string) =
                match hit.getAttribute name with
                | null
                | "" -> None
                | v -> Some v

            match attr "data-wikilink", attr "data-tag", attr "data-external" with
            | Some note, _, _ -> onLink note
            | _, Some tag, _ -> onTag tag
            | _, _, Some url -> onExternal url
            | _ -> ()

    Html.div [
        prop.className "plinth-preview max-w-none"
        prop.onClick onClick
        prop.dangerouslySetInnerHTML (renderNote content noteExists)
    ]
