//! The sample notebook offered on the welcome screen.
//!
//! It is not a tutorial: it is a small, real vault made of ordinary
//! Markdown files, written so that reading it means using the features it
//! describes. Today's note links to the welcome note, which therefore has
//! a backlink the moment the notebook opens; the tags are real tags; the
//! Firmament has enough stars and lines to look like a sky rather than a
//! diagram. Nothing here is special-cased by the app — a reader can
//! delete every file and be left with a working empty vault.

use std::fs;
use std::path::{Path, PathBuf};

/// The note the welcome screen opens after seeding.
pub const LANDING_NOTE: &str = "Start here";

/// Folder name under the user's Documents directory.
pub const FOLDER_NAME: &str = "Plinth Sample";

/// The sample notebook's files, as (note name, body). `{today}` in a body
/// is replaced with the daily note's name so the links resolve on any day.
fn notes(today: &str) -> Vec<(String, String)> {
    vec![
        (
            LANDING_NOTE.to_string(),
            format!(
                r#"# Start here

This is a note. It is a plain Markdown file in a folder on your computer,
and it is nothing else — you could open it in Notepad and lose nothing.

Everything in this sample is a real note. Edit it, break it, delete it.

## Three things to try

1. Press **Ctrl+D**. That opens [[{today}]], today's note. The same
   keystroke every day, so capture never costs you a decision about where
   something goes.
2. Type `[[` on a blank line below. A menu of every note in this folder
   opens at the cursor. Choose one and you have made a link — and if you
   type a name that doesn't exist yet, Plinth makes that note when you
   click it.
3. Press **Ctrl+G** to open [[The Firmament]].

## Then read these

- [[How Plinth works]] — the folder, the index, and what Plinth refuses to do
- [[Ideas]] — a scratch list, tagged so you can find it again

Look at the **Backlinks** panel on the right. Today's note points at this
one, so it appears there. You never filed it anywhere; structure like that
accumulates on its own.

#plinth
"#
            ),
        ),
        (
            today.to_string(),
            format!(
                r#"# {today}

This is your daily note. Ctrl+D opens it from anywhere, creating it the
first time. Most days that is the only navigation you need.

- Read [[Start here]]
- Skim [[How Plinth works]]
- [ ] Try typing `[[` somewhere in this line
- [ ] Open [[The Firmament]] with Ctrl+G

A thought worth keeping: the tools that outlast their makers are the ones
that never asked for anything. #plinth
"#
            ),
        ),
        (
            "How Plinth works".to_string(),
            r#"# How Plinth works

## The folder is the app

A vault is one folder of Markdown files. The `.md` files at the top level
are your notes — one file, one note, and the filename is the note's name.
That is the whole data model. There is no database holding your words
hostage, and no export button to plan an escape through.

Subfolders are yours. Plinth leaves them completely alone, even if they
contain Markdown, and says so once rather than quietly half-reading them.

The one thing Plinth adds is a `.plinth` folder holding a search index.
Delete it whenever you like; it is rebuilt from your files the next time
you open the notebook, because *the files are the source of truth*.

## Links

A name inside double square brackets links one note to another, the way
`[[The Firmament]]` does here. A link to a note that doesn't exist yet is
not an error — it is a note you haven't written. Click it and it exists.

Rename a note and every link to it is rewritten across the whole notebook.
Prose that happens to mention the old name is left alone.

## Tags

A `#tag` anywhere in a note files it under that tag. See the tag list at
the bottom of the sidebar. Tags cut across the notebook; links connect it.
Use whichever matches how you actually think.

## What it refuses

- No plugins, ever. Nothing to install, nothing to rot.
- No account, no cloud, no telemetry.
- No proprietary format.

Deleting a note sends the file to the Recycle Bin, not into thin air.

## Free, and open

Plinth is MIT licensed and the source is at
<https://github.com/sebbejones/plinth>. Click that link: it opens in your
browser, and Plinth is still here behind it. A link in a note never
replaces the app you were reading it in.

#plinth
"#
            .to_string(),
        ),
        (
            "The Firmament".to_string(),
            r#"# The Firmament

Press **Ctrl+G**.

Every note in the notebook is a star. Every link is a line between two of
them. Daily notes burn amber. A link to a note nobody has written
yet shows as a dim *unborn* star — the shape of an idea before it has any
words in it.

Drag it around. Zoom. Click a star to open that note.

It is not a filing system and it will not tidy anything for you. It is a
way of seeing the shape of what you have been thinking about, which is
usually different from the shape you assumed.

*Firmament*, in the old sense: the vault of heaven. The other kind of
vault.

See also [[How Plinth works]] and [[An unborn note]] — that second one has
no file behind it yet, which is why it is drawn dim.

#plinth
"#
            .to_string(),
        ),
        (
            "Ideas".to_string(),
            format!(
                r#"# Ideas

A scratch note, so the tag list has more than one entry in it.

- [ ] Move this sample somewhere permanent, or delete it
- [ ] Point Plinth at a folder of notes I already have
- [x] Open Plinth for the first time

Anything can be a note: a reading list, a decision I want to remember the
reasoning behind, the shape of an argument. See [[{today}]] for today.

> The best time to start keeping notes was years ago. The second best time
> is the note you are looking at.

#ideas #plinth
"#
            ),
        ),
    ]
}

/// Create the sample notebook under `parent`, returning its folder.
///
/// If the folder already holds Markdown files it is left untouched — a
/// second visit to the welcome screen must never overwrite notes someone
/// has been writing in. Returns whether anything was written.
pub fn ensure_sample(parent: &Path, today: &str) -> Result<(PathBuf, bool), String> {
    let root = parent.join(FOLDER_NAME);
    if has_notes(&root) {
        return Ok((root, false));
    }
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    for (name, body) in notes(today) {
        fs::write(root.join(format!("{name}.md")), body).map_err(|e| e.to_string())?;
    }
    Ok((root, true))
}

fn has_notes(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.path().is_file()
            && e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("md"))
                == Some(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plinth-sample-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn seeds_a_notebook_whose_links_all_resolve() {
        let parent = temp_dir("seed");
        let (root, created) = ensure_sample(&parent, "2026-08-05").unwrap();
        assert!(created);

        let names: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.path().file_stem().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&LANDING_NOTE.to_string()));
        assert!(names.contains(&"2026-08-05".to_string()));

        // Every [[link]] points at a note that exists — except the one
        // deliberately left unborn so the Firmament has a dim star. Asked
        // of the real parser, not a lookalike regex, so the sample is
        // checked against the rules the app actually applies.
        for (_, body) in notes("2026-08-05") {
            for target in crate::link_parser::extract_links(&body) {
                assert!(
                    names.contains(&target) || target == "An unborn note",
                    "dangling link: {target}"
                );
            }
        }
        assert!(crate::link_parser::extract_tags(
            &fs::read_to_string(root.join("Ideas.md")).unwrap()
        )
        .contains(&"ideas".to_string()));

        // The landing note is linked to, so it opens with a backlink.
        let daily = fs::read_to_string(root.join("2026-08-05.md")).unwrap();
        assert!(daily.contains(&format!("[[{LANDING_NOTE}]]")));

        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn an_existing_sample_is_never_overwritten() {
        let parent = temp_dir("keep");
        let (root, _) = ensure_sample(&parent, "2026-08-05").unwrap();
        fs::write(root.join(format!("{LANDING_NOTE}.md")), "my own words").unwrap();

        let (again, created) = ensure_sample(&parent, "2026-08-06").unwrap();
        assert_eq!(again, root);
        assert!(!created);
        assert_eq!(
            fs::read_to_string(root.join(format!("{LANDING_NOTE}.md"))).unwrap(),
            "my own words"
        );
        // ...and no note was added for the new day either.
        assert!(!root.join("2026-08-06.md").exists());

        fs::remove_dir_all(&parent).ok();
    }
}
