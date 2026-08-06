# Plinth — Next Steps

Status as of 2026-08-04: **v0.3.1 is published** — the note-creation and
linking pass below, tested in the real desktop build and released.

## v0.3.1 — the note-creation and linking pass (2026-08-04)

Prompted by Sebbe noticing there was no visible way to start a new note
after finishing one. All four changes are the same shape: the machinery
already existed, the affordance didn't.

- **New note** as a real button in the sidebar (now the primary action,
  with Today beside it), plus `Ctrl+N` and a palette entry. The dialog
  refuses `/ \ : ..` and, for a name that already exists, changes its
  button from Create to Open rather than erroring.
- **`[[` opens a note menu at the cursor.** The biggest of the four:
  links are the backbone of backlinks and the Firmament, but linking
  previously meant recalling a note's exact spelling. Arrow/click to
  choose, Enter or Tab to accept, `]]` closed automatically. The palette's
  ranking moved to `Utils/Fuzzy.fs` so both matchers behave identically.
  Caret position is measured with a hidden mirror div — a textarea exposes
  no caret geometry and this one soft-wraps. That measurement is the most
  fragile part of the pass; if the menu ever sits wrong, look there first.
- **`Ctrl+S` saves and acknowledges it** ("saved ✓" for ~1.4 s). Autosave
  already made this unnecessary for safety; silence after a universal
  habit read as data loss in an app selling durability. Autosave and
  Ctrl+S now share one `performSave`, so neither can bypass the v0.3.0
  conflict checks.
- **Keyboard shortcuts are discoverable** — ⚙ → "Keyboard shortcuts…"
  and a palette entry. Previously every shortcut was listed only in the
  palette, which you had to know a shortcut to open.

Verified in browser dev mode: full round trip of typing `[[tyr`, picking
the suggestion, autosave, and the backlink appearing on the target note;
the exists/illegal-name branches of the New note dialog; the saved ✓
sequence via a DOM observer. Clean Fable compile, no console errors.

**Also exercised in the real 0.3.1 desktop build** against a scratch
vault (2026-08-04): the New note button and dialog created a real file on
disk; `Ctrl+N` opened the dialog; the `[[` menu rendered at the caret and
still tracked it correctly at Large font size; the accepted link was
written to the `.md` file and the backlink appeared on the target note;
⚙ → Keyboard shortcuts rendered. 19 Rust tests pass.

One thing was *not* photographed in the real app: the "saved ✓" flash
lasts ~1.4 s, which is shorter than a screenshot round trip, so every
capture caught the settled "saved" state. `Ctrl+S` reaching the app is
established (`Ctrl+N` goes through the identical window listener) and the
flash itself was verified in dev mode with a DOM observer — but if you
want to see it, press `Ctrl+S` yourself and watch the header.

A trap for next time: `open_application "Plinth"` launches the *installed*
build, not the one in `src-tauri/target/release/`. Two instances ran
side by side and the first round of "testing" was against v0.3.0 without
the new features. Check `(Get-Process plinth).Path` before trusting what
is on screen.

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

**DONE 2026-07-30.** Pushed, tagged `v0.3.0`, released on GitHub as
"v0.3.0: the Foundation Holds" with `Plinth_0.3.0_x64-setup.exe` attached.

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

## v0.4 — "ready to share", not "more powerful" (planned 2026-08-05)

Sebbe's framing, and the whole scope test: v0.4 makes Plinth safe to hand to
someone who has never heard the word "vault". Nothing here adds power.

1. **Deletion is recoverable.** `delete_file` in `src-tauri/src/lib.rs` is a
   bare `fs::remove_file`; move it to the `trash` crate so deleted notes go
   to the Windows Recycle Bin. Trap: network shares and some removable
   drives have no Recycle Bin, so the fallback must *ask* rather than
   silently hard-delete.
2. **A real Markdown renderer.** `Utils/Markdown.fs` is 126 lines of regex
   handling headings, bullets, `**bold**`, `` `code` ``, `[[links]]` and
   `#tags` — and nothing else. Replace with markdown-it (`html: false`,
   plus DOMPurify as belt-and-braces) for italics, strikethrough, ordered
   and nested lists, checklists, blockquotes, fenced code and tables. "No
   plugins" means no marketplace, not no internal libraries. The real work
   is not the parser: the current renderer returns React elements with
   `onClick` handlers, so wiki links and tags become a custom inline rule
   emitting `data-` attributes, read by one delegated click handler.
3. **Ordinary web links, which do not work at all today.** Not "render
   badly" — `[text](url)` renders as literal text; there is no link
   handling anywhere in the codebase. In a Tauri app an `http://` link must
   open in the *system browser* or it navigates the WebView away and the
   app is gone with no back button. Needs `tauri-plugin-opener`, a scheme
   allowlist (http/https/mailto only; `javascript:` and `file:` blocked),
   and a capabilities entry. Folded into item 2's release but tracked
   separately because it is its own job.
4. **DONE 2026-08-05. Fix the first ten minutes.** The welcome screen now
   asks one question — where do your notes live? — and offers three
   answers (`welcome` in `Pages/NoteView.fs`); the philosophy is a
   footnote under them. The sample notebook is five real notes written in
   `src-tauri/src/sample.rs` and seeded into `Documents\Plinth Sample` by
   the new `create_sample_vault` command; an existing sample is opened as
   the user left it, never overwritten, and they are told so once. A test
   walks every `[[link]]` in the sample through the app's own link parser,
   which is what caught the sample teaching the syntax in a way that
   created spurious ghost stars in the Firmament — inline code spans are
   not exempt from link extraction, so `` `[[Example]]` `` in a note is a
   real link. The flat-model warning is now a **notice**: `VaultScan`
   carries `warnings` (act on this) and `notices` (this is working as
   designed) separately, and notices render in quiet stone rather than
   alarm amber, because "Plinth left your subfolders alone" is the promise
   being kept, not a fault. Still to do from this item: the sample content
   has not been used for the landing-page screenshots yet.

   *Original entry:* The welcome screen explains the
   philosophy and then asks for a vault folder — fine for someone who
   already knows what a vault is. Replace with three paths: create a new
   Plinth notebook, open an existing Markdown folder, or try a sample
   notebook. **Decided: the sample lands in `Documents\Plinth Sample`** —
   a real folder they can find again, which reinforces the premise that
   notes are plain files on their disk, not a demo mode. If it already
   exists, offer to open it. The sample teaches through use — today's
   note, a wiki link, a backlink, a tag, the Firmament — with no tutorial
   carousel. Two related first-run gaps: "open an existing folder" will
   often hit nested files or duplicate basenames and fire the v0.3.0
   flat-model warning, which must read as an explanation of how Plinth
   works rather than a defect report; and the sample content doubles as the
   landing-page screenshot set, so write it once and use it twice.
5. **Automatic updates — dropped, and replaced.** The Microsoft Store
   handles updates and forbids self-updating apps, so shipping v0.4 through
   the Store (see item 1 under Still open) covers this for free.
   `tauri-plugin-updater` stays out.

One further gap, not in the original four: **there is no error story.**
v0.3.0 handles the *expected* failures well — conflicts, external deletion,
watcher death — but a genuine panic, or a vault on a drive that got
unplugged, has no defined surface. For "ready to share", an unexpected
error has to be legible rather than a blank window.

What v0.4 is **not**, written down because scope will try to grow: no
editor rewrite (still textarea + preview), no sync, no mobile, no export
beyond the existing zip, no incremental indexer.

## Still open

1. Code signing (removes the "Windows protected your PC" warning).
   **Route decided 2026-08-05: the Microsoft Store, via MSIX.** Sebbe
   surfaced this; it is better than either certificate option.

   What was ruled out first: **Azure Trusted Signing is off the table** —
   since 2025-04-02 Microsoft restricted new signups to *organizations* in
   the US/Canada with three years of verifiable history, and individual
   onboarding is paused. The paid fallback would have been a Certum Open
   Source cert (~€69 first year, ~€29 renewal), but an OV certificate does
   not silence SmartScreen immediately — it accrues reputation over
   downloads. Only EV (several hundred a year) bypasses it on day one.

   The Store route beats both, for free:
   - Store developer registration is free for individuals *and* companies
     (May 2026), ~200 markets, no credit card.
   - **Microsoft signs the MSIX on submission.** No certificate purchase,
     and Store installs never show SmartScreen.
   - Microsoft publishes a Tauri-specific MSIX guide using the `winapp` CLI
     (learn.microsoft.com/windows/apps/dev-tools/winapp-cli/guides/tauri,
     dated 2026-07-23). It produces a real MSIX, not a listing that links
     to a download.
   - **It also deletes item 4 (auto-update)** — the Store updates the app,
     and Store policy forbids self-updating, so `tauri-plugin-updater`
     must stay OUT of the MSIX build.
   - Checked the main risk: full-trust MSIX is **not** sandboxed. Plinth's
     arbitrary-folder file access, bundled SQLite and the `notify` watcher
     keep working as in a normal desktop app.

   Known costs before starting: certification is a human review (screenshots,
   description, age rating, privacy statement, rejection loops); an
   individual account publishes under Sebbe's verified legal name; the
   package identity and publisher must come from a **Partner Center
   reservation made before packaging**, or the app gets packaged twice;
   `winapp` needs Windows 11 and is new enough to expect rough edges. Two
   channels persist — the Store build is clean, the GitHub `.exe` still
   warns — which argues for making the Store the primary link on the
   landing page. One small side effect: the remembered vault path lives in
   WebView2 localStorage, which is package-local, so someone moving from
   the NSIS install to the Store install lands on the welcome screen again.
2. ~~Make the build stand on its own.~~ **DONE 2026-08-05.** It turned out
   the environment half was already true: the persistent user PATH carries
   `%LOCALAPPDATA%\Microsoft\dotnet` and `%USERPROFILE%\.cargo\bin`, with
   .NET SDK 8.0.422 + 10.0.301, Rust 1.96.1 and Node 24 all resolving in a
   plain terminal. Only the scripts still reached into Claude's app storage.
   `build.cmd` and `dev.ps1` were rewritten to require the normal toolchain
   on PATH and fail with install links if it is missing — no app-storage
   paths remain. Verified by running `build.cmd` in a scrubbed environment
   (registry PATH only, `DOTNET_ROOT` removed): exit 0, both bundles
   produced, `Plinth_0.3.1_x64-setup.exe` included.
3. Genuine future work surfaced by v0.3.0, none urgent: an incremental
   (rather than rebuild-on-open) index if big-vault open time ever matters
   in practice; treating an external rename as a rename instead of
   delete+create if the OS event quality allows it.

(The old `.plinth/.plinth/` check from this list is done — cause found,
fixed, and regression-tested in v0.3.0.)

## How to pick up next session

Open this file. v0.3.1 is out and `main` is clean — nothing is pending.
The "Still open" list is what remains, and none of it is urgent.
