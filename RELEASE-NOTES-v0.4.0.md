# Plinth v0.4.0 — Ready to Share

This release makes Plinth nothing more powerful. It makes it safe to hand
to someone who has never heard the word "vault".

Everything up to now was built for someone who already understood what a
plain-text notebook is and had decided they wanted one. The first screen
explained a philosophy and then asked for a folder. Deleting a note
destroyed the file. The preview handled about a third of the Markdown
people actually write, and a link to a web page was rendered as text
because clicking one would have replaced the app with the page and left no
way back.

Those are the things that decide whether someone is still using a tool ten
minutes after opening it, and this release is all four of them.

## What's new

**Three ways to start.** The first screen asks one question — where do
your notes live? — and offers three answers: create a new notebook, open a
folder of Markdown you already have, or try a sample notebook. The
argument for Plinth is a footnote underneath, where it belongs; someone
who has just installed it has been convinced enough already, and what they
need next is a way in.

**A sample notebook, which is not a tutorial.** Five real notes written
into `Documents\Plinth Sample`, arranged so that reading them means using
what they describe. Today's note links to the first page, so a backlink is
already sitting in the panel when the notebook opens — nobody had to
explain what a backlink is. The Firmament has a dim unwritten star in it
because one note deliberately links somewhere nothing has been written
yet. There is no carousel, nothing to click through, and no demo mode: it
is an ordinary folder of Markdown files you can edit, break, or delete,
and if you delete all of them you are left with a working empty notebook.

**Deleting a note is recoverable.** Notes go to the Recycle Bin, where you
can restore them, instead of being destroyed. Drives that have no Recycle
Bin — network shares, some removable media — are handled by asking rather
than quietly deleting anyway: Plinth reports that it *hasn't* deleted
anything, and permanent deletion becomes a separate, explicit choice with
its own confirmation.

**A real Markdown preview.** CommonMark, via a proper parser bundled into
the app, plus tables, strikethrough, task lists and fenced code. Italics,
ordered and nested lists, blockquotes and horizontal rules all render.
`[[Wiki links]]` and `#tags` still work inside it and are still clickable,
and broken links are still drawn differently from live ones.

Raw HTML in a note is shown as text and never rendered — so a note
containing `<img src=x onerror=...>` displays those characters and does
nothing else, whether you wrote it or pasted it from somewhere you
shouldn't have.

This does not contradict the no-plugins promise. "No plugins" means no
marketplace and nothing for you to install, troubleshoot, or have rot out
from under you. It never meant refusing to use a library.

**Web links open in your browser.** `[text](https://…)` and bare URLs are
now real links, and clicking one hands it to your system browser while
Plinth stays exactly where it was. Only `http`, `https` and `mailto` are
allowed through, checked in three independent places, because a note is
your content and this is the one place it reaches outside the app.

**Files in subfolders are explained, not reported as a fault.** Plinth
reads notes from the top level of your folder and leaves subfolders
completely alone. It always said so; it said so in alarm colours, which
meant the first thing someone saw after pointing Plinth at notes they
already had was what looked like an error about their own files. That
message is now a quiet notice that leads with what Plinth did rather than
what it skipped. Warnings — two files claiming one note name, say — still
look like warnings, because those you actually have to do something about.

## Compatibility

- No file format changes, no index changes, nothing to migrate.
- Nothing was removed; every existing shortcut still does what it did.
- Upgrading is just installing over the top. Your vault, your last-opened
  note, and your theme and font-size settings are all preserved.
- The sample notebook is only ever created if you ask for it, and if a
  folder is already there it is opened as you left it, never overwritten.

## Known limitations

- Relative links to other files, like `[notes](./notes.md)`, render as
  plain text. Only absolute `http`/`https`/`mailto` URLs are treated as
  links. Use `[[Wiki links]]` to link between notes.
- The link index does not know about code spans, so writing `` `[[Example]]` ``
  in a note — quoting the syntax rather than using it — still counts as a
  link and will put an unwritten star in the Firmament. The preview gets
  this right and renders it as code; only the index is fooled.
- The sample notebook goes to your Documents folder as Windows reports it.
  If you have OneDrive folder redirection turned on, that means the sample
  will sync. Your own notebooks go wherever you point Plinth.
- An unexpected internal error still has no dedicated screen. Expected
  failures — conflicts, external deletion, a watcher that can't start —
  are all handled explicitly, but a genuine crash is not yet legible.
- Everything under v0.3.0's known limitations still applies: the vault is
  flat by design, external renames appear as delete-plus-create, and
  opening very large vaults re-reads every file.

## Windows SmartScreen

The installer is unsigned, so Windows SmartScreen may warn on first run.
Choose "More info", then "Run anyway".

Code signing is still on the list, and the route is now decided: Plinth
will be distributed through the Microsoft Store as an MSIX package, which
is signed by Microsoft and installs without the warning. That also settles
the question of automatic updates — the Store handles them, and Plinth
will not ship a self-updater of its own.
