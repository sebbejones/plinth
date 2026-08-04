/// Name matching shared by the command palette and the [[link]] menu, so
/// the same query ranks the same way wherever you type it.
module Plinth.Utils.Fuzzy

/// Prefix beats substring beats subsequence; a longer contiguous run
/// inside a subsequence match ranks it higher ("plnth" finds "Plinth").
/// 0 means no match at all.
let score (query: string) (name: string) =
    let q = query.ToLowerInvariant()
    let n = name.ToLowerInvariant()

    if n.StartsWith q then 120
    elif n.Contains q then 90
    else
        let mutable qi = 0
        let mutable streak = 0
        let mutable best = 0

        for ch in n do
            if qi < q.Length && ch = q.[qi] then
                qi <- qi + 1
                streak <- streak + 1
                best <- max best streak
            else
                streak <- 0

        if qi = q.Length && q.Length > 0 then 40 + best else 0

/// Best `limit` names for `query`, strongest first.
let rank (limit: int) (query: string) (names: string[]) =
    names
    |> Array.choose (fun n ->
        match score query n with
        | 0 -> None
        | s -> Some(s, n))
    |> Array.sortByDescending fst
    |> Array.truncate limit
    |> Array.map snd
