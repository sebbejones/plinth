# Plinth v0.3.0 — The Foundation Holds

The filesystem and Plinth now stay in agreement. Close and reopen the app,
edit your notes in another program while Plinth is running, or rename a note
outright — nothing is silently lost, overwritten, or left stale.

## What's new

**Plinth reopens your last vault.** On startup the app returns to the vault
you had open and lands on the note you left off in. If the folder has since
been moved, deleted, or is on a drive that isn't connected, Plinth returns
to the welcome screen with a plain explanation — it never recreates a
missing folder and never guesses.

**Outside edits are noticed while the app runs.** A filesystem watcher keeps
the index, sidebar, tags, backlinks, search, and the Firmament in step with
Markdown files created, changed, deleted, or renamed by any other program.
Plinth's own autosaves are recognised and skipped, so nothing loops. If
watching can't start (unusual filesystems, permissions), the vault still
opens and a notice explains that outside edits won't appear until reopen.

**Conflicts are explicit, never silent.** What happens when the file under
your open note changes on disk:

- *Your note has no unsaved edits, the file changed*: the new disk content
  simply loads. No ceremony.
- *Your note has unsaved edits, the file changed*: autosave stops, both
  versions are preserved, and a banner lets you compare them and choose —
  keep your edits (overwriting the file) or load the disk version
  (discarding your edits). Each button says exactly what is destroyed.
- *No unsaved edits, the file was deleted*: Plinth says so and offers a
  deliberate "recreate" — it never quietly resurrects the file.
- *Unsaved edits, the file was deleted*: your text is held in memory,
  autosave will not recreate the file behind your back, and you choose to
  recreate the note from your text or let it go.

**Notes can be renamed.** A Rename button in the editor (also in the
command palette) renames the file and rewrites every `[[wiki link]]` to it
across the vault — matching links case-insensitively (including accented
letters, which Windows also folds in filenames), leaving plain prose that
merely mentions the old name untouched. Unsafe names (path characters,
reserved Windows device names, leading dots, `.plinth`) and collisions with
existing notes are rejected with clear errors. Case-only renames
(`ideas` → `Ideas`) work correctly on Windows.

**The vault model is now explicit: flat.** The `.md` files at the vault
root are the notes. Subfolders were previously walked and nested files
indexed by bare filename, which silently collapsed duplicate names into one
note. Now nested Markdown files are left untouched and reported once,
dismissibly, so nothing is half-indexed behind your back. Files whose names
differ only by case are likewise surfaced instead of silently merged.

**Faster indexing.** Vault opening now indexes in a single SQLite
transaction: a 1,000-note vault opens in a few seconds instead of the
better part of a minute.

**`.plinth` is fenced off.** The internal index folder can no longer be
selected as a vault (which previously created `.plinth/.plinth/`), is never
watched, indexed, or exported, and is the only thing Plinth ever adds to
your folder.

## Compatibility and migration

- No file format changes. Your Markdown is untouched — as always.
- The index inside `.plinth/` is rebuilt automatically; nothing to migrate.
- If your vault relied on **nested** Markdown files being indexed, they no
  longer are (they were previously indexed unreliably, with silent
  name-collision loss). Move a file to the vault root to make it a note.
  Plinth lists the affected files when the vault opens.

## Known limitations

- An external rename is seen as delete-plus-create (that is what the OS
  reports), so a renamed open note shows the "deleted externally" state;
  the renamed file appears as a new note in the sidebar.
- Opening very large vaults re-reads every file: roughly 4 s for 1,000
  notes, 10 s for 5,000 on a mid-range machine, dominated by per-file disk
  reads. Search stays fast (well under 200 ms at 5,000 notes).
- The vault is flat by design: subfolders are never notes.

## Windows SmartScreen

The installer is unsigned, so Windows SmartScreen may warn on first run.
Choose "More info", then "Run anyway". Code signing remains on the list.
