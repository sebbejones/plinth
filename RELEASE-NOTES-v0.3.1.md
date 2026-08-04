# Plinth v0.3.1 — Visible Doors

Everything in this release already worked. None of it was findable.

You could always create a note, and you could always link one note to
another — but the only way in was a keyboard shortcut you had to know
about first. This release gives those actions buttons, menus, and a place
to look them up.

## What's new

**A New note button.** It sits in the sidebar, where it should have been
all along, and is now the primary action with Today beside it. `Ctrl+N`
and the command palette do the same thing. The dialog rejects names that
aren't valid filenames, and if the name already exists it offers to open
that note instead of refusing.

**Typing `[[` offers your notes.** A menu appears at the cursor listing
notes that match what you type, ranked the same way the command palette
ranks them — `[[tyr` finds "The Tyranny of the Marketplace". Arrow keys or
the mouse to choose, Enter or Tab to accept, and the closing `]]` is added
for you.

This is the change that matters most. Links are what backlinks and the
Firmament are built from, but making one previously meant remembering
exactly how you had spelled a note's name. Get it slightly wrong and you
silently created a link to a different, non-existent note.

**`Ctrl+S` saves, and says so.** Plinth has always autosaved shortly after
you stop typing, and still does — this changes nothing about whether your
work is safe. But `Ctrl+S` is a thirty-year habit, and a keystroke that
did nothing at all was the wrong answer in an app whose whole promise is
that your words are safe. It now writes immediately and the header
confirms with "saved ✓".

**Keyboard shortcuts are written down in the app.** Under ⚙ → "Keyboard
shortcuts…", and in the command palette. Previously every shortcut was
listed only inside the palette, which you needed to know a shortcut to
open.

## Compatibility

- No file format changes, no index changes, nothing to migrate.
- Nothing was removed and no existing shortcut changed.
- Upgrading is just installing over the top.

## Known limitations

- The `[[` menu is positioned by measuring where the caret is, which a
  textarea does not report directly. In unusual cases (very long unbroken
  lines) it may sit a line off.
- Everything listed under v0.3.0's known limitations still applies: the
  vault is flat by design, external renames appear as delete-plus-create,
  and opening very large vaults re-reads every file.

## Windows SmartScreen

The installer is unsigned, so Windows SmartScreen may warn on first run.
Choose "More info", then "Run anyway". Code signing remains on the list.
