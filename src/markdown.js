// Markdown rendering for the preview pane.
//
// This replaces a hand-rolled regex renderer that could only manage
// headings, bullets, bold and inline code. markdown-it and DOMPurify are
// bundled into Plinth — they are part of the product, not a marketplace,
// and nothing here is fetched or user-extensible.
//
// Plinth adds three things on top of CommonMark, all as real markdown-it
// rules rather than string surgery, so they can't fire inside code blocks:
//   [[Wiki Link]]  -> a link that opens (or creates) a note
//   #tag           -> a link that filters the sidebar by tag
//   - [ ] checklist items
// Everything else — italics, strikethrough, ordered and nested lists,
// blockquotes, fenced code, tables — is stock markdown-it.

import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";

const md = new MarkdownIt({
  // Raw HTML in a note is rendered as literal text, never as markup. The
  // DOMPurify pass below is the second line of defence, not the first.
  html: false,
  linkify: true,
  breaks: false,
});

// Only schemes that make sense from a notes app, and never javascript: or
// file:. markdown-it's default validator is more permissive than we want.
const SAFE_SCHEME = /^(https?:\/\/|mailto:)/i;
md.validateLink = (url) => SAFE_SCHEME.test(String(url).trim());

// Set per render. Single-threaded, and render() is synchronous, so a module
// -level slot is safe and saves threading a context through every rule.
let noteExists = () => false;

const esc = (s) =>
  String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

const LINK_LIVE =
  "cursor-pointer text-emerald-700 underline decoration-emerald-300 hover:text-emerald-900 dark:text-emerald-400 dark:decoration-emerald-700 dark:hover:text-emerald-300";
// Links to notes that don't exist yet render muted with a dashed underline —
// still clickable, and clicking creates them.
const LINK_MISSING =
  "cursor-pointer text-stone-400 underline decoration-dashed decoration-stone-300 hover:text-emerald-700 dark:text-stone-500 dark:decoration-stone-600 dark:hover:text-emerald-400";
const TAG_CLS =
  "cursor-pointer text-amber-700 hover:text-amber-900 dark:text-amber-400 dark:hover:text-amber-300";

// --- [[wiki links]] ---------------------------------------------------
// `[` is one of markdown-it's terminator characters, so an inline rule
// placed before `link` gets a chance at it.

md.inline.ruler.before("link", "wikilink", (state, silent) => {
  const { src, pos } = state;
  if (src.charCodeAt(pos) !== 0x5b || src.charCodeAt(pos + 1) !== 0x5b) {
    return false;
  }
  const end = src.indexOf("]]", pos + 2);
  if (end < 0) return false;

  const target = src.slice(pos + 2, end).trim();
  // Same shape the link parser and the [[ menu accept: no nesting.
  if (!target || /[[\]]/.test(target)) return false;

  if (!silent) {
    state.push("wikilink", "", 0).content = target;
  }
  state.pos = end + 2;
  return true;
});

md.renderer.rules.wikilink = (tokens, idx) => {
  const name = tokens[idx].content;
  const cls = noteExists(name) ? LINK_LIVE : LINK_MISSING;
  return `<a class="${cls}" data-wikilink="${esc(name)}">${esc(name)}</a>`;
};

// --- #tags ------------------------------------------------------------
// `#` is NOT a terminator character, so an inline rule would never be
// reached — the `text` rule swallows it. Split text tokens after the fact
// instead, the way markdown-it's own linkifier does.

const TAG_RE = /(^|\s)#([A-Za-z][A-Za-z0-9_/-]*)/g;

md.core.ruler.push("plinth_tags", (state) => {
  for (const block of state.tokens) {
    if (block.type !== "inline") continue;

    let linkDepth = 0;
    const out = [];
    let changed = false;

    for (const token of block.children) {
      // Don't turn a #fragment inside a link's text into a tag.
      if (token.type === "link_open") linkDepth++;
      if (token.type === "link_close") linkDepth--;
      if (token.type !== "text" || linkDepth > 0) {
        out.push(token);
        continue;
      }

      TAG_RE.lastIndex = 0;
      let last = 0;
      let m;
      let hit = false;
      while ((m = TAG_RE.exec(token.content)) !== null) {
        const start = m.index + m[1].length;
        if (start > last) {
          const before = new state.Token("text", "", 0);
          before.content = token.content.slice(last, start);
          out.push(before);
        }
        const tag = new state.Token("plinth_tag", "", 0);
        tag.content = m[2];
        out.push(tag);
        last = m.index + m[0].length;
        hit = true;
      }

      if (!hit) {
        out.push(token);
        continue;
      }
      changed = true;
      if (last < token.content.length) {
        const rest = new state.Token("text", "", 0);
        rest.content = token.content.slice(last);
        out.push(rest);
      }
    }

    if (changed) block.children = out;
  }
});

md.renderer.rules.plinth_tag = (tokens, idx) => {
  const tag = tokens[idx].content;
  // The filter is case-insensitive; the note keeps whatever case was typed.
  return `<a class="${TAG_CLS}" data-tag="${esc(
    tag.toLowerCase()
  )}">#${esc(tag)}</a>`;
};

// --- checklists -------------------------------------------------------
// `- [ ] item` / `- [x] item`. Rendered as real, disabled checkboxes: the
// preview shows state, the Markdown file stays the place you change it.

md.core.ruler.push("plinth_checklists", (state) => {
  const tokens = state.tokens;
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i].type !== "list_item_open") continue;

    const inline = tokens[i + 2];
    if (!inline || tokens[i + 1].type !== "paragraph_open") continue;
    if (inline.type !== "inline") continue;

    const m = /^\[([ xX])\]\s+/.exec(inline.content);
    if (!m) continue;

    const checked = m[1] !== " ";
    inline.content = inline.content.slice(m[0].length);
    const first = inline.children && inline.children[0];
    if (first && first.type === "text") {
      first.content = first.content.slice(m[0].length);
    }

    const box = new state.Token("plinth_checkbox", "", 0);
    box.meta = { checked };
    inline.children = [box, ...(inline.children || [])];

    tokens[i].attrJoin("class", "plinth-task");
  }
});

md.renderer.rules.plinth_checkbox = (tokens, idx) =>
  `<input type="checkbox" disabled${
    tokens[idx].meta.checked ? " checked" : ""
  }> `;

// --- external links ---------------------------------------------------
// Marked so the click handler can hand them to the OS browser. Letting the
// WebView navigate would replace the app with the page, with no way back.

const defaultLinkOpen =
  md.renderer.rules.link_open ||
  ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));

md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  const href = tokens[idx].attrGet("href");
  if (href && SAFE_SCHEME.test(href)) {
    tokens[idx].attrSet("data-external", href);
    tokens[idx].attrJoin("class", LINK_LIVE);
  }
  return defaultLinkOpen(tokens, idx, options, env, self);
};

/**
 * Render a note body to sanitized HTML.
 * @param {string} content the raw Markdown
 * @param {(name: string) => boolean} exists whether a wiki link resolves
 */
export function renderNote(content, exists) {
  noteExists = typeof exists === "function" ? exists : () => false;
  try {
    return DOMPurify.sanitize(md.render(content ?? ""), {
      // data-* survive by default; these are the non-data extras we emit.
      ADD_ATTR: ["target", "rel"],
      // No <form>, no <style>, and nothing that can navigate on its own.
      FORBID_TAGS: ["style", "form", "iframe", "object", "embed"],
    });
  } finally {
    noteExists = () => false;
  }
}
