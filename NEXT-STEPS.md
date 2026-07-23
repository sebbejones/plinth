# Plinth — Next Steps

Status as of 2026-07-23: **v0.3.0 — "The Foundation Holds" — is BUILT and
committed locally on branch `release/v0.3.0`, awaiting Sebbe's push, tag,
and GitHub release.** v0.2.0 remains the published release until then.

## v0.3.0 — what was done (2026-07-23)

The trust-and-continuity release: the user-owned filesystem and Plinth stay
in agreement.

- **Last vault reopens on startup** (path in localStorage; on failure the
  welcome screen explains instead of guessing; `.plinth` is rejected as a
  vault choice, which also kills the old `.plinth/.plinth/` sighting —
  regression-tested).
- **Filesystem watcher** (`notify` crate, non-recursive by design since the
  vault is flat) keeps the index/sidebar/tags/backlinks/search/Firmament in
  step with external create/modify/delete/rename. Plinth's own saves are
  recognised by content hash and skipped. Watcher startup/runtime failure
  degrades to a dismissible notice, never a crash.
- **Explicit conflict handling** — see RELEASE-NOTES-v0.3.0.md for the
  four cases (clean/dirty × modified/deleted). Saves are conflict-checked
  in the backend (`write_file` takes the hash of the last-seen disk state),
  so even a save racing an external edit cannot silently clobber it.
- **Safe rename** with vault-wide `[[link]]` rewriting, strict name
  validation, collision rejection, case-only rename support.
- **Folder-model decision: explicitly FLAT.** Root-level `*.md` files are
  the notes. Rationale: the v0.2 code walked subfolders but identified
  notes by bare filename stem, so duplicate basenames silently collapsed
  into one note (last file walked won) — data-loss-shaped and unfixable
  without path-identity complexity (folder UI, link disambiguation) that
  contradicts "deliberately small." Nested Markdown files and duplicate
  basenames are now detected, left untouched, and surfaced as dismissible
  warnings. Documented in README ("How the vault works").
- **Indexing made transactional** after measurement showed ~57 s to open a
  1,000-note vault (per-insert fsync). Now ~4 s for 1,000 notes / ~10 s for
  5,000 (dominated by per-file reads); search 11–147 ms, Firmament graph
  data 18–158 ms, single-file watcher refresh 16–93 ms (release build,
  this machine, generated realistic vaults).
- 19 Rust unit/integration tests (was 3), including a real-watcher
  end-to-end test; conflict/deleted/rename UI exercised in browser dev mode
  AND in the real built app (driven over the WebView2 devtools protocol);
  versions bumped to 0.3.0 everywhere (package.json, tauri.conf.json,
  Cargo.toml).
- An independent fresh-context review caught two real bugs before release,
  both fixed with regression tests: (1) SQLite's built-in NOCASE is
  ASCII-only while Windows folds accented letters in filenames, so
  [[über notes]] missed the indexed "Über Notes" and create-on-miss could
  stub over the real file — NOCASE is now overridden with Unicode-aware
  collation; (2) a Plinth-side delete-then-recreate could mask a later
  genuinely external deletion for up to 15 s. Real-app testing separately
  caught that Tauri rejects commands with plain strings, which Fable's
  `ex.Message` turned into silently swallowed errors — normalized via
  `Tauri.errorText` at every catch site.

Known limitations (also in the release notes): external rename appears as
delete+create; big-vault open time scales with file count; the flat model
is a feature, not a bug.

## To publish v0.3.0 (Sebbe)

```
git push -u origin release/v0.3.0     # or merge to main first
git tag v0.3.0
git push origin v0.3.0
```

Then draft the GitHub release from RELEASE-NOTES-v0.3.0.md and upload
`src-tauri/target/release/bundle/nsis/Plinth_0.3.0_x64-setup.exe`.

## Decided 2026-07-18 (Sebbe): Plinth is FREE and open source

Draft B adopted as the README (with the v0.2.0 features folded in), MIT
LICENSE added, landing byline now links to sebbejones.com. Both draft files
deleted after adoption; `demo-vault/` gitignored (local screenshot content).
For the record, draft A's paid-product idea (one-time purchase + optional
"Plinth Sync" subscription) is parked, not lost: revisit only if Plinth ever
outgrows its role as free evidence for the Teach AI Your Business course.

## Decided 2026-07-19 (Sebbe): one identity, everything under sebbejones

The GitHub account was renamed from `SignalTheoryCo` to `sebbejones`, display
name to "Sebbe Jones". SignalTheoryCo was never public anywhere: it only ever
appeared as the account name and the commit email. Old repo URLs redirect, so
the v0.2.0 download link keeps working either way. Links in this file, the
README, and `landing/index.html` were updated to the new account.

## Where the landing page goes

**Decision: `sebbejones.com/plinth`, as a page on the existing site.** Not
GitHub Pages. sebbejones.com already deploys from the `sebbejones-site` repo to
Netlify, and that site already uses this exact pattern for proof pages
(`/hotels`, `/workshop`, `/course`). Plinth is proof material for the Teach AI
Your Business course, so it belongs in that set.

A GitHub Pages workflow was built and then removed on 2026-07-19: it would have
added a second hosting system next to Netlify for no gain, and it would have put
the page on a github.io address instead of the domain.

The page itself is verified good. Served locally from `landing/`, every asset
loaded, no console errors, all sections rendered.

**DONE 2026-07-20.** The page is live at https://sebbejones.com/plinth/,
rebuilt in the site's brand system (cream/Georgia, dark hero band, sticky nav)
in the `sebbejones-site` repo (`plinth/index.html` plus the three screenshots
in `plinth/assets/`). Footer links added on the homepage, /workshop, and
/course; llms.txt gained a "Proof: Plinth" section; the page carries
SoftwareApplication schema. `landing/index.html` in this repo stays as the
standalone original; the site copy is the deployed one, so style edits should
happen there.

## Still open

1. Decide on code signing (removes the "Windows protected your PC" warning).
   Azure Trusted Signing is the current low-cost route. Not a blocker — the
   landing page already explains the warning to first-time downloaders.
2. Make the build stand on its own: install the .NET SDK and Rust as normal
   system tools so `build.cmd` doesn't depend on Claude's app storage.
3. Genuine future work surfaced by v0.3.0, none urgent: an incremental
   (rather than rebuild-on-open) index if big-vault open time ever matters
   in practice; treating an external rename as a rename instead of
   delete+create if the OS event quality allows it.

(The old `.plinth/.plinth/` check from this list is done — cause found,
fixed, and regression-tested in v0.3.0.)

## How to pick up next session

Open this file. v0.3.0 is built and committed on `release/v0.3.0` but NOT
pushed — publication (push, tag, GitHub release, installer upload) is
deliberately left to Sebbe; the exact commands are above. After that, the
"Still open" list is what remains.
