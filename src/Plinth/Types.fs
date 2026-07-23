module Plinth.Types

/// Metadata for a note in the vault, as returned by the Rust index.
type NoteMeta = { Name: string; Path: string }

/// A note's canonical name, its markdown body, and the hash of that body —
/// the editor keeps the hash as "the disk state I last saw" so saves can
/// detect external edits.
type NoteContent = { Name: string; Content: string; Hash: string }

/// What opening a vault returned: notes, scan warnings (nested markdown
/// files, duplicate names), and whether filesystem watching started.
type VaultInfo =
    { Notes: NoteMeta[]
      Warnings: string[]
      WatcherError: string option }

/// Outcome of a conflict-checked save. Status is "saved" (Notes + Hash are
/// fresh), "conflict" (Disk holds the current file content), or "missing"
/// (the file was deleted externally; nothing was written).
type WriteResult =
    { Status: string
      Notes: NoteMeta[]
      Hash: string
      Disk: string }

/// Outcome of a rename: fresh note list, the canonical new name, and how
/// many [[links]] across the vault were rewritten.
type RenameOutcome =
    { Notes: NoteMeta[]
      Name: string
      LinksUpdated: int }

/// One batch of external filesystem changes, in note names.
type VaultChanges =
    { Created: string[]
      Modified: string[]
      Deleted: string[]
      Warnings: string[] }

/// One full-text search result.
type SearchHit = { Name: string; Snippet: string }

/// A tag and how many notes carry it.
type TagCount = { Tag: string; Count: int }

/// One star in the Firmament. Exists = false marks a "ghost": a link
/// target no note file backs yet.
type GraphNode =
    { Name: string
      Exists: bool
      Tags: string[]
      Degree: int }

/// One [[link]] between two notes, canonical names on both ends.
type GraphEdge = { Source: string; Target: string }

/// The whole vault as nodes + edges, as returned by the Rust index.
type GraphData =
    { Nodes: GraphNode[]
      Edges: GraphEdge[] }

/// Everything the main pane can be showing, as one discriminated union —
/// impossible states (an editor with no note, a note without a vault)
/// simply cannot be represented. The two conflict states keep the unsaved
/// local text in memory and halt autosave until the user chooses.
type NoteState =
    | NoVault
    | VaultLoading
    | LoadingNote of name: string
    | EditingNote of name: string * content: string
    /// The note has unsaved local edits AND its file changed on disk.
    | ConflictNote of name: string * local: string * disk: string
    /// The note's file was deleted outside Plinth. `local` holds unsaved
    /// text if there was any; None means the note was clean.
    | DeletedNote of name: string * local: string option
    | NoteError of message: string
