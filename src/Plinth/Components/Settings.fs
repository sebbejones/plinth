/// Gear-button popover: vault path, font size, theme, vault export.
module Plinth.Components.Settings

open Feliz
open Plinth.Hooks.UseSettings

let private row (label: string) (children: ReactElement list) =
    Html.div [
        prop.className "flex items-center justify-between gap-3 px-4 py-2"
        prop.children [
            Html.span [ prop.className "text-xs text-stone-500 dark:text-stone-400"; prop.text label ]
            Html.div [ prop.className "flex gap-1"; prop.children children ]
        ]
    ]

let private segButton (active: bool) (label: string) (onClick: unit -> unit) =
    Html.button [
        prop.className (
            "rounded px-2 py-0.5 text-xs "
            + (if active then
                   "bg-emerald-800 text-white"
               else
                   "bg-stone-200 text-stone-600 hover:bg-stone-300 dark:bg-stone-700 dark:text-stone-300 dark:hover:bg-stone-600")
        )
        prop.onClick (fun _ -> onClick ())
        prop.text label
    ]

/// Every shortcut in one place. The command palette lists most of them
/// too, but only once you already know Ctrl+K — so this is reachable with
/// the mouse alone.
let private shortcuts =
    [ "Ctrl+N", "New note"
      "Ctrl+D", "Today's daily note"
      "Ctrl+K", "Command palette"
      "Ctrl+G", "The Firmament"
      "Ctrl+S", "Save now (Plinth also autosaves)"
      "[[", "Link to a note — a menu of names appears"
      "Esc", "Close whatever is open" ]

let ShortcutsDialog (onClose: unit -> unit) =
    Html.div [
        prop.className "absolute inset-0 z-50 bg-black/30"
        prop.onClick (fun _ -> onClose ())
        prop.children [
            Html.div [
                prop.className
                    "mx-auto mt-[18vh] w-full max-w-sm rounded-xl border border-stone-200 bg-white p-5 shadow-2xl dark:border-stone-600 dark:bg-stone-800"
                prop.onClick (fun e -> e.stopPropagation ())
                prop.children [
                    Html.h2 [
                        prop.className "font-serif text-lg font-semibold text-stone-800 dark:text-stone-100"
                        prop.text "Keyboard shortcuts"
                    ]
                    Html.div [
                        prop.className "mt-3 flex flex-col gap-1.5"
                        prop.children (
                            shortcuts
                            |> List.map (fun (keys, what) ->
                                Html.div [
                                    prop.key keys
                                    prop.className "flex items-baseline justify-between gap-4"
                                    prop.children [
                                        Html.span [
                                            prop.className "text-sm text-stone-600 dark:text-stone-300"
                                            prop.text what
                                        ]
                                        Html.kbd [
                                            prop.className
                                                "flex-none rounded border border-stone-300 bg-stone-100 px-1.5 py-0.5 font-mono text-xs text-stone-600 dark:border-stone-600 dark:bg-stone-900 dark:text-stone-300"
                                            prop.text keys
                                        ]
                                    ]
                                ])
                        )
                    ]
                    Html.div [
                        prop.className "mt-4 flex justify-end"
                        prop.children [
                            Html.button [
                                prop.className
                                    "rounded bg-emerald-800 px-3 py-1 text-sm text-white hover:bg-emerald-700"
                                prop.onClick (fun _ -> onClose ())
                                prop.text "Close"
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ]

[<ReactComponent>]
let SettingsMenu
    (settings: SettingsApi)
    (vault: string option)
    (onChangeVault: unit -> unit)
    (onExport: unit -> unit)
    (onShortcuts: unit -> unit)
    =
    let isOpen, setOpen = React.useState false

    Html.div [
        prop.className "relative"
        prop.children [
            Html.button [
                prop.className
                    "rounded px-2 py-1 text-sm text-stone-500 hover:bg-stone-200 dark:text-stone-400 dark:hover:bg-stone-700"
                prop.title "Settings"
                prop.onClick (fun _ -> setOpen (not isOpen))
                prop.text "⚙"
            ]
            if isOpen then
                Html.div [
                    prop.className
                        "absolute left-0 top-8 z-20 w-64 rounded-lg border border-stone-200 bg-white py-2 shadow-lg dark:border-stone-700 dark:bg-stone-800"
                    prop.children [
                        Html.p [
                            prop.className "truncate px-4 pb-2 text-xs text-stone-400 dark:text-stone-500"
                            prop.title (vault |> Option.defaultValue "")
                            prop.text (
                                match vault with
                                | Some path -> "Vault: " + path
                                | None -> "No vault selected"
                            )
                        ]
                        row "Font size" [
                            segButton (settings.FontSize = Small) "S" (fun () -> settings.SetFontSize Small)
                            segButton (settings.FontSize = Medium) "M" (fun () -> settings.SetFontSize Medium)
                            segButton (settings.FontSize = Large) "L" (fun () -> settings.SetFontSize Large)
                        ]
                        row "Theme" [
                            segButton (settings.Theme = Light) "Light" (fun () -> settings.SetTheme Light)
                            segButton (settings.Theme = Dark) "Dark" (fun () -> settings.SetTheme Dark)
                        ]
                        Html.div [
                            prop.className "mt-1 border-t border-stone-200 px-4 pt-2 dark:border-stone-700"
                            prop.children [
                                Html.button [
                                    prop.className
                                        "block w-full rounded px-2 py-1.5 text-left text-sm text-stone-600 hover:bg-stone-100 dark:text-stone-300 dark:hover:bg-stone-700"
                                    prop.onClick (fun _ ->
                                        setOpen false
                                        onShortcuts ())
                                    prop.text "Keyboard shortcuts…"
                                ]
                                Html.button [
                                    prop.className
                                        "block w-full rounded px-2 py-1.5 text-left text-sm text-stone-600 hover:bg-stone-100 dark:text-stone-300 dark:hover:bg-stone-700"
                                    prop.onClick (fun _ ->
                                        setOpen false
                                        onChangeVault ())
                                    prop.text "Change vault folder…"
                                ]
                                Html.button [
                                    prop.className
                                        "block w-full rounded px-2 py-1.5 text-left text-sm text-stone-600 hover:bg-stone-100 dark:text-stone-300 dark:hover:bg-stone-700"
                                    prop.onClick (fun _ ->
                                        setOpen false
                                        onExport ())
                                    prop.text "Export vault as .zip…"
                                ]
                            ]
                        ]
                    ]
                ]
        ]
    ]
