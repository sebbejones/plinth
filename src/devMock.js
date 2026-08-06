// Browser-only dev harness. When the app runs in a plain browser (no
// Tauri shell), this installs a mock `__TAURI_INTERNALS__.invoke` backed
// by an in-memory vault, so `npm run dev` + a browser tab is enough to
// work on the frontend. Inside the real desktop app Tauri injects the
// genuine internals before any module runs, so this guard makes the
// whole file a no-op there.
if (!("__TAURI_INTERNALS__" in window)) {
  const seed = {
    "Plinth": `# Plinth

A notebook, not an ecosystem.

Plinth stores notes as [[Markdown]] files in a folder I choose. No accounts, no plugins, no marketplace. The point is [[Owning Your Data]], everything else is detail.

What it does well: daily notes, wiki links, backlinks, tags, fast search. What it will never do: grow a plugin store.

#plinth #principle
`,
    "Plain Text": `# Plain Text

Plain text outlives the software that made it. A [[Markdown]] file written today opens in anything, on any machine, in twenty years.

That durability is the quiet case for [[Owning Your Data]]. Rich formats promise features and charge you in lock-in.

#principle #writing
`,
    "Markdown": `# Markdown

Markdown is plain text with light structure. Headings, links, lists, emphasis, nothing you cannot read with your own eyes in a raw file.

It is the format [[Plinth]] stores everything in. Not a database, not a proprietary blob. Just \`.md\` files you can search, back up, and move.

#writing
`,
    "Owning Your Data": `# Owning Your Data

A note you cannot open without an account is not really yours. It is borrowed.

The test is simple. If the company disappears tonight, do you still have your words tomorrow? With [[Plain Text]] the answer is yes. The files sit in a folder on your disk. No export step, no format to escape, nothing to lose.

This is the principle everything else follows from. See [[The Tyranny of the Marketplace]] for what happens when a tool forgets it.

#principle #plinth
`,
    "Reading List": `# Reading List

Books worth the second read.

- *The Shallows*, Nicholas Carr. What the web does to attention.
- *How to Take Smart Notes*, Sonke Ahrens. The case for linking over filing.
- *A Pattern Language*, Christopher Alexander. Structure that grows from small parts, the way backlinks and [[Owning Your Data]] do.

Currently reading the Ahrens. Notes land in the daily note first, then get promoted.

#reading
`,
    "The Tyranny of the Marketplace": `# The Tyranny of the Marketplace

Every calm tool eventually grows a plugin store. Then the calm is gone.

The marketplace is not added for you. It is added because it makes the tool sticky and the company money. Soon your setup depends on ten strangers' code, and one of them stops maintaining theirs.

This is the failure [[Owning Your Data]] is meant to prevent. A tool for thinking should ask nothing of you and take nothing from you.

#idea #principle
`,
    "Signal Theory": `# Signal Theory

The company behind [[Plinth]]. Solo, small, software that respects the person using it.

Current focus:
- Ship the [[Plinth]] README rewrite
- Build a small base of users before charging anything
- Write more in public, log the loose ideas in the daily note

#project
`,
    "Roadmap": `# Roadmap

What [[Plinth]] earns next, in order:

- The Firmament. See the whole vault as a sky. Done when it feels alive.
- The command palette. [[Plinth]] should never make me reach for the mouse.
- Maybe [[Print Stylesheets]] someday. Maybe [[Sync I Will Never Build]] never.

#plinth #roadmap
`,
    "2026-07-03": `# 2026-07-03

Started sketching the graph view on paper. Every note a star, every link a line. [[Plinth]] deserves a sky.

#daily #plinth
`,
    "2026-07-05": `# 2026-07-05

Slow morning. Started mapping out why [[Owning Your Data]] is the whole point, not a feature.

- Draft the README refusal section
- Take the folder screenshot
- Read a chapter, log it in [[Reading List]]

#daily #plinth
`,
  };

  const linkRe = /\[\[([^\[\]]+)\]\]/g;
  const tagRe = /(?:^|\s)#([A-Za-z][A-Za-z0-9_/-]*)/g;

  const notes = new Map(); // lower name -> { name, content }
  for (const [name, content] of Object.entries(seed)) {
    notes.set(name.toLowerCase(), { name, content });
  }
  const recents = new Map(); // canonical name -> opened_at
  let clock = 1;

  const extract = (re, content, map) => {
    const out = [];
    for (const m of content.matchAll(re)) {
      const v = map(m[1]);
      if (v && !out.includes(v)) out.push(v);
    }
    return out;
  };
  const linksOf = (c) => extract(linkRe, c, (s) => s.trim());
  const tagsOf = (c) => extract(tagRe, c, (s) => s.toLowerCase());

  const list = () =>
    [...notes.values()]
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
      .map((n) => ({ Name: n.name, Path: "/demo/" + n.name + ".md" }));

  const isSubsequence = (needle, hay) => {
    let i = 0;
    for (const ch of hay) if (ch === needle[i]) i++;
    return i >= needle.length;
  };

  // Mirrors the Rust content hash's role (any deterministic function of
  // the content works for the mock).
  const hashOf = (s) => {
    let h = 2166136261 >>> 0;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619) >>> 0;
    }
    return h.toString(16);
  };

  const commands = {
    "plugin:dialog|open": () => "Demo Vault (in-memory)",
    // plugin-dialog's confirm() is a wrapper over the `message` command: it
    // returns the label of the button that was clicked and compares it to
    // "Ok". Returning anything else here reads as Cancel, which silently
    // turns every confirmed action in the browser demo into a no-op.
    // (There is no `plugin:dialog|confirm` command — don't add one back.)
    "plugin:dialog|message": () => "Ok",
    "plugin:dialog|save": () => null,
    // The real app hands web links to the OS browser; in the demo just log,
    // so a stray click can't navigate the page away from the app.
    "plugin:opener|open_url": ({ url }) => {
      console.info("devMock would open externally:", url);
      return null;
    },
    // The real app pushes watcher events; the browser demo has no
    // filesystem, so listeners are registered and never fire.
    "plugin:event|listen": () => 1,
    "plugin:event|unlisten": () => null,

    set_vault: () => ({
      Notes: list(),
      Warnings: [],
      // One notice, so the quiet explanatory banner is visible in the
      // browser demo without having to build a folder to trigger it.
      Notices: [
        "Your subfolders are yours — Plinth left them exactly as they are, including 2 Markdown file(s) inside them (Archive/Old.md, Archive/Older.md). Notes come from the top level of this folder only, so move a file up here if you want Plinth to treat it as a note.",
      ],
      WatcherError: null,
    }),

    // There is no filesystem here, so the demo vault stands in for the
    // sample folder. A stub landing note is added so the welcome screen's
    // "open the sample, land on its first page" path is exercisable in the
    // browser — the real content lives in src-tauri/src/sample.rs.
    create_sample_vault: () => {
      if (!notes.has("start here")) {
        notes.set("start here", {
          name: "Start here",
          content:
            "# Start here\n\nStand-in for the sample notebook's landing note. " +
            "The real one is written by src-tauri/src/sample.rs.\n\n" +
            "- [[Owning Your Data]]\n- [ ] Try typing `[[` here\n\n#plinth\n",
        });
      }
      return { Path: "C:\\Users\\demo\\Documents\\Plinth Sample", Created: true };
    },
    read_dir: () => list(),

    read_file: ({ name }) => {
      const key = name.trim().toLowerCase();
      let note = notes.get(key);
      if (!note) {
        note = { name: name.trim(), content: `# ${name.trim()}\n\n` };
        notes.set(key, note);
      }
      recents.set(note.name, clock++);
      return { Name: note.name, Content: note.content, Hash: hashOf(note.content) };
    },

    load_note: ({ name }) => {
      const note = notes.get(name.trim().toLowerCase());
      return note ? { Name: note.name, Content: note.content, Hash: hashOf(note.content) } : null;
    },

    write_file: ({ name, content, baseHash }) => {
      const key = name.trim().toLowerCase();
      const existing = notes.get(key);
      if (baseHash != null) {
        if (!existing) return { Status: "missing", Notes: [], Hash: "", Disk: "" };
        if (hashOf(existing.content) !== baseHash && existing.content !== content)
          return { Status: "conflict", Notes: [], Hash: "", Disk: existing.content };
      }
      notes.set(key, { name: existing ? existing.name : name.trim(), content });
      return { Status: "saved", Notes: list(), Hash: hashOf(content), Disk: "" };
    },

    rename_note: ({ oldName, newName }) => {
      const oldKey = oldName.trim().toLowerCase();
      const trimmed = newName.trim();
      const newKey = trimmed.toLowerCase();
      if (!trimmed) throw "Note name is empty.";
      if (/[/\\:*?"<>|]/.test(trimmed) || trimmed.includes("..") || trimmed.startsWith("."))
        throw `Note names can't contain unsafe characters.`;
      const note = notes.get(oldKey);
      if (!note) throw `Note "${oldName}" was not found.`;
      if (newKey !== oldKey && notes.has(newKey)) throw `A note named "${trimmed}" already exists.`;
      const linkRe2 = new RegExp(
        `\\[\\[\\s*${note.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\]\\]`,
        "gi",
      );
      let linksUpdated = 0;
      for (const [k, n] of notes.entries()) {
        const rewritten = n.content.replace(linkRe2, () => {
          linksUpdated++;
          return `[[${trimmed}]]`;
        });
        if (rewritten !== n.content) notes.set(k, { name: n.name, content: rewritten });
      }
      const moved = notes.get(oldKey);
      notes.delete(oldKey);
      notes.set(newKey, { name: trimmed, content: moved.content });
      if (recents.has(note.name)) {
        const t = recents.get(note.name);
        recents.delete(note.name);
        recents.set(trimmed, t);
      }
      return { Notes: list(), Name: trimmed, LinksUpdated: linksUpdated };
    },

    delete_file: ({ name, permanent }) => {
      const key = name.trim().toLowerCase();
      const note = notes.get(key);
      if (note) recents.delete(note.name);
      notes.delete(key);
      // There is no Recycle Bin in a browser; report the real app's happy path.
      return { Status: permanent ? "deleted" : "recycled", Notes: list() };
    },

    search: ({ query }) => {
      const q = query.trim().toLowerCase();
      if (!q) return [];
      const terms = q.split(/\s+/);
      const compact = q.replace(/\s+/g, "");
      const scored = [];
      for (const { name, content } of notes.values()) {
        const nameL = name.toLowerCase();
        const contentL = content.toLowerCase();
        let score = nameL.includes(q)
          ? 100
          : terms.every((t) => nameL.includes(t))
            ? 80
            : compact.length >= 3 && isSubsequence(compact, nameL)
              ? 55
              : 0;
        if (terms.every((t) => contentL.includes(t))) score += 35;
        if (score > 0) {
          const pos = contentL.indexOf(terms.find((t) => contentL.includes(t)) ?? "");
          const snippet =
            pos >= 0
              ? content.slice(Math.max(0, pos - 40), pos + 60).replace(/\n/g, " ")
              : content.slice(0, 80);
          scored.push([score, { Name: name, Snippet: snippet }]);
        }
      }
      scored.sort((a, b) => b[0] - a[0] || a[1].Name.localeCompare(b[1].Name));
      return scored.slice(0, 50).map(([, hit]) => hit);
    },

    get_backlinks: ({ name }) => {
      const target = name.trim().toLowerCase();
      return [...notes.values()]
        .filter((n) => linksOf(n.content).some((l) => l.toLowerCase() === target))
        .map((n) => n.name)
        .sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
    },

    get_graph: () => {
      const ghosts = new Map(); // lower -> display
      const seen = new Set();
      const edges = [];
      for (const { name, content } of notes.values()) {
        for (const raw of linksOf(content)) {
          const lower = raw.toLowerCase();
          if (lower === name.toLowerCase()) continue;
          const real = notes.get(lower);
          if (!real && !ghosts.has(lower)) ghosts.set(lower, raw);
          const target = real ? real.name : ghosts.get(lower);
          const key = name.toLowerCase() + " " + lower;
          if (!seen.has(key)) {
            seen.add(key);
            edges.push({ Source: name, Target: target });
          }
        }
      }
      const degree = new Map();
      for (const e of edges) {
        for (const end of [e.Source.toLowerCase(), e.Target.toLowerCase()]) {
          degree.set(end, (degree.get(end) ?? 0) + 1);
        }
      }
      const nodes = [
        ...[...notes.values()].map((n) => ({
          Name: n.name,
          Exists: true,
          Tags: tagsOf(n.content),
          Degree: degree.get(n.name.toLowerCase()) ?? 0,
        })),
        ...[...ghosts.entries()].map(([lower, display]) => ({
          Name: display,
          Exists: false,
          Tags: [],
          Degree: degree.get(lower) ?? 0,
        })),
      ];
      return { Nodes: nodes, Edges: edges };
    },

    get_tags: () => {
      const counts = new Map();
      for (const n of notes.values()) {
        for (const t of tagsOf(n.content)) counts.set(t, (counts.get(t) ?? 0) + 1);
      }
      return [...counts.entries()]
        .sort((a, b) => a[0].localeCompare(b[0]))
        .map(([tag, count]) => ({ Tag: tag, Count: count }));
    },

    get_notes_by_tag: ({ tag }) =>
      [...notes.values()]
        .filter((n) => tagsOf(n.content).includes(tag.toLowerCase()))
        .map((n) => n.name)
        .sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" })),

    get_recents: () =>
      [...recents.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 10)
        .map(([name]) => name),

    export_vault: () => {
      throw "Export needs the desktop app — this is the in-browser demo.";
    },
  };

  // Debug handle so browser-devtools (and UI tests) can simulate external
  // edits: mutate a note here, then keep typing — the next autosave sees
  // the mismatch and the conflict flow kicks in.
  window.__plinthMock = { notes, hashOf };

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args = {}) => {
      const handler = commands[cmd];
      if (!handler) throw `devMock: no handler for command "${cmd}"`;
      return handler(args);
    },
    transformCallback: (cb) => cb,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  };

  console.info("Plinth devMock active: in-memory vault, no Tauri backend.");
}
