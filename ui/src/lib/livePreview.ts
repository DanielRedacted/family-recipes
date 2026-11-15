// Formatted editing, Apple Notes style: the document stays plain Markdown and is saved exactly as
// typed, but the markup (#, **, >, [ ](url)…) is never shown. Decorations hide it,
// the cursor steps over it (atomic ranges), and deleting one half of a pair removes the other
// half too, so formatting is removed cleanly instead of leaving stray symbols. Headings are sized,
// bullets become dots, task boxes become checkboxes, images show inline. Tables are drawn by
// codemirror-markdown-tables (Editor.svelte), with this extension running inside each cell.
import { syntaxTree } from '@codemirror/language';
import { EditorState, RangeSet, Transaction, type Range } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';

export interface LivePreviewOptions {
  /** URL for an image reference as written in the note (or null to leave it as text) */
  imageUrl: (src: string) => string | null;
  /** open a [[link]] target (⌘-click) */
  openLink: (target: string) => void;
  /** open a Markdown link's address (⌘-click): a web page or a file in the recipes folder */
  openHref: (href: string) => void;
}

class Bullet extends WidgetType {
  eq() { return true; }
  toDOM() { const s = document.createElement('span'); s.className = 'cm-lp-bullet'; s.textContent = '•'; return s; }
}

class Checkbox extends WidgetType {
  constructor(readonly checked: boolean, readonly pos: number) { super(); }
  eq(o: Checkbox) { return o.checked === this.checked && o.pos === this.pos; }
  toDOM(view: EditorView) {
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = this.checked;
    box.className = 'cm-lp-task';
    box.setAttribute('aria-label', this.checked ? 'Done' : 'Not done');
    box.onmousedown = (e) => e.preventDefault();
    box.onclick = (e) => {
      e.preventDefault();
      view.dispatch({ changes: { from: this.pos + 1, to: this.pos + 2, insert: this.checked ? ' ' : 'x' } });
    };
    return box;
  }
  ignoreEvent() { return true; }
}

class Img extends WidgetType {
  constructor(readonly src: string, readonly alt: string) { super(); }
  eq(o: Img) { return o.src === this.src && o.alt === this.alt; }
  toDOM() {
    const img = document.createElement('img');
    img.src = this.src;
    img.alt = this.alt;
    img.className = 'cm-lp-img';
    return img;
  }
}

/** "↗" after a link to a web page, as in the reading view */
class ExtMark extends WidgetType {
  eq() { return true; }
  toDOM() { const s = document.createElement('span'); s.className = 'cm-lp-extmark'; s.textContent = '↗'; s.title = 'A web page: ⌘-click to open it in the browser'; return s; }
}

class Rule extends WidgetType {
  eq() { return true; }
  toDOM() { const s = document.createElement('span'); s.className = 'cm-lp-hr'; return s; }
}

const hide = Decoration.replace({});
const bullet = Decoration.replace({ widget: new Bullet() });
const rule = Decoration.replace({ widget: new Rule() });
const mark = (cls: string) => Decoration.mark({ class: cls });
const line = (cls: string) => Decoration.line({ class: cls });
const WIKI = /\[\[([^\]\n]+)\]\]/g;
const PAIRS = /^(StrongEmphasis|Emphasis|Strikethrough|InlineCode)$/;
const PAIR_MARK: Record<string, string> = { StrongEmphasis: 'EmphasisMark', Emphasis: 'EmphasisMark', Strikethrough: 'StrikethroughMark', InlineCode: 'CodeMark' };

const isReplace = (d: Decoration) => (d.spec as { widget?: unknown; block?: boolean }).widget !== undefined || d === hide;

// ---------- inline markup ----------

function build(view: EditorView, opts: LivePreviewOptions): { deco: DecorationSet; atomic: DecorationSet } {
  const { state } = view;
  const out: Range<Decoration>[] = [];
  const atomic: Range<Decoration>[] = [];
  const doc = state.doc;
  const push = (d: Decoration, from: number, to = from) => {
    if (to < from || (to === from && isReplace(d) && d === hide)) return;
    out.push(d.range(from, to));
    if (isReplace(d) && to > from) atomic.push(d.range(from, to));
  };
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter: (n) => {
        const name = n.name;
        const h = /^(ATX|Setext)Heading(\d)$/.exec(name);
        if (h) { out.push(line(`cm-lp-h${h[2]}`).range(doc.lineAt(n.from).from)); return; }
        switch (name) {
          case 'Frontmatter':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-fm').range(doc.line(l).from));
            return false;
          case 'HeaderMark': {
            const end = doc.lineAt(n.from).to;
            // the "## " before a heading, or the closing "##" / setext underline
            if (n.to < end && n.from === doc.lineAt(n.from).from + (doc.lineAt(n.from).text.length - doc.lineAt(n.from).text.trimStart().length)) push(hide, n.from, Math.min(n.to + 1, end));
            else push(hide, n.from, n.to);
            return;
          }
          case 'EmphasisMark':
          case 'CodeMark':
          case 'StrikethroughMark':
            if (n.node.parent?.name !== 'FencedCode') push(hide, n.from, n.to);
            return;
          case 'StrongEmphasis': out.push(mark('cm-lp-strong').range(n.from, n.to)); return;
          case 'Emphasis': out.push(mark('cm-lp-em').range(n.from, n.to)); return;
          case 'Strikethrough': out.push(mark('cm-lp-strike').range(n.from, n.to)); return;
          case 'InlineCode': out.push(mark('cm-lp-code').range(n.from, n.to)); return;
          case 'FencedCode':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-codeblock').range(doc.line(l).from));
            return;
          case 'Blockquote':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-quote').range(doc.line(l).from));
            return;
          case 'QuoteMark':
            push(hide, n.from, Math.min(n.to + (doc.sliceString(n.to, n.to + 1) === ' ' ? 1 : 0), doc.lineAt(n.from).to));
            return;
          case 'HorizontalRule':
            push(rule, n.from, n.to);
            return;
          case 'ListMark': {
            const text = doc.sliceString(n.from, n.to);
            if (n.node.parent?.getChild('Task')) return; // the checkbox covers it
            const space = doc.sliceString(n.to, n.to + 1) === ' ' ? 1 : 0;
            if (/^[-*+]$/.test(text)) push(bullet, n.from, n.to + space);
            else out.push(mark('cm-lp-listmark').range(n.from, n.to));
            return;
          }
          case 'TaskMarker': {
            const checked = /x/i.test(doc.sliceString(n.from, n.to));
            const lm = n.node.parent?.parent?.getChild('ListMark');
            const start = lm ? lm.from : n.from;
            const end = Math.min(n.to + (doc.sliceString(n.to, n.to + 1) === ' ' ? 1 : 0), doc.lineAt(n.to).to);
            push(Decoration.replace({ widget: new Checkbox(checked, n.from) }), start, end);
            if (checked && end < doc.lineAt(n.to).to) out.push(mark('cm-lp-done').range(end, doc.lineAt(n.to).to));
            return;
          }
          case 'Image': {
            const url = n.node.getChild('URL');
            if (!url) return;
            const src = opts.imageUrl(doc.sliceString(url.from, url.to).replace(/^<|>$/g, ''));
            const alt = doc.sliceString(n.from + 2, doc.sliceString(n.from, n.to).indexOf(']') + n.from);
            if (src) push(Decoration.replace({ widget: new Img(src, alt) }), n.from, n.to);
            return false;
          }
          case 'Link': {
            const url = n.node.getChild('URL');
            const ext = !!url && /^<?https?:/i.test(doc.sliceString(url.from, url.to));
            out.push(mark(ext ? 'cm-lp-link cm-lp-ext' : 'cm-lp-link').range(n.from, n.to));
            if (ext) out.push(Decoration.widget({ widget: new ExtMark(), side: 1 }).range(n.to));
            // hide "[" and "](url)" — keep the text
            const marks = n.node.getChildren('LinkMark');
            if (marks.length >= 2) {
              push(hide, marks[0].from, marks[0].to);
              push(hide, marks[1].from, n.to);
            }
            return;
          }
        }
      },
    });
    // [[wiki links]] are not Markdown syntax: find them in the text
    const text = doc.sliceString(from, to);
    for (const m of text.matchAll(WIKI)) {
      const a = from + m.index!;
      const b = a + m[0].length;
      out.push(mark('cm-lp-wikilink').range(a, b));
      const bar = m[1].indexOf('|');
      push(hide, a, a + 2 + (bar >= 0 ? bar + 1 : 0));
      push(hide, b - 2, b);
    }
  }
  // an empty pair just made by the B / I buttons ("**|**"): hide it while the cursor is inside
  for (const r of state.selection.ranges) {
    if (!r.empty) continue;
    const p = r.head;
    for (const w of [2, 1]) {
      const pair = doc.sliceString(p - w, p + w);
      if (pair === '*'.repeat(2 * w) || pair === '~'.repeat(2 * w)) { out.push(hide.range(p - w, p), hide.range(p, p + w)); break; }
    }
  }
  return { deco: RangeSet.of(out, true), atomic: RangeSet.of(atomic, true) };
}

// ---------- deleting half of a pair removes the other half ----------

/** codemirror-markdown-tables copying the note's text into a table cell (undo, redo…): not typing */
// reads the package's private annotation values ("table.edit.undo"…); recheck when upgrading it
const fromTableSync = (tr: Transaction) =>
  ((tr as unknown as { annotations?: { value: unknown }[] }).annotations ?? []).some((a) => typeof a.value === 'string' && a.value.startsWith('table.'));

/**
 * After a deletion, a bold/italic/strike/code span, link or [[link]] that lost one of its hidden
 * marks — or all of its text — has its remaining marks removed too.
 */
function pairCleanup(tr: Transaction) {
  if (!tr.docChanged || !tr.isUserEvent('delete') || fromTableSync(tr)) return tr;
  const start = tr.startState;
  const extra: { from: number; to: number }[] = [];
  const covers = (a: number, b: number, f: number, t: number) => f <= a && t >= b;
  tr.changes.iterChangedRanges((fA, tA) => {
    if (tA <= fA) return;
    syntaxTree(start).iterate({
      from: fA, to: tA,
      enter: (n) => {
        let head: { from: number; to: number } | null = null;
        let tail: { from: number; to: number } | null = null;
        if (PAIRS.test(n.name)) {
          const ms = n.node.getChildren(PAIR_MARK[n.name]);
          if (ms.length < 2) return;
          head = { from: ms[0].from, to: ms[0].to };
          tail = { from: ms[ms.length - 1].from, to: ms[ms.length - 1].to };
        } else if (n.name === 'Link') {
          const ms = n.node.getChildren('LinkMark');
          if (ms.length < 2) return;
          head = { from: ms[0].from, to: ms[0].to };
          tail = { from: ms[1].from, to: n.to };
        } else return;
        const hd = covers(head.from, head.to, fA, tA), tl = covers(tail.from, tail.to, fA, tA);
        const textGone = fA <= head.to && tA >= tail.from;
        if (hd && !tl) extra.push(tail);
        else if (tl && !hd) extra.push(head);
        else if (!hd && !tl && textGone) extra.push(head, tail);
      },
    });
    const l = start.doc.lineAt(fA);
    for (const m of l.text.matchAll(WIKI)) {
      const a = l.from + m.index!, b = a + m[0].length;
      const bar = m[1].indexOf('|');
      const head = { from: a, to: a + 2 + (bar >= 0 ? bar + 1 : 0) }, tail = { from: b - 2, to: b };
      const hd = covers(head.from, head.to, fA, tA), tl = covers(tail.from, tail.to, fA, tA);
      if (hd && !tl) extra.push(tail);
      else if (tl && !hd) extra.push(head);
    }
  });
  if (!extra.length) return tr;
  const changes = extra
    .filter((r, i, all) => all.findIndex((x) => x.from === r.from && x.to === r.to) === i)
    .map((r) => ({ from: tr.changes.mapPos(r.from, 1), to: tr.changes.mapPos(r.to, -1) }))
    .filter((r) => r.to > r.from);
  return changes.length ? [tr, { changes, sequential: true, userEvent: tr.annotation(Transaction.userEvent) }] : tr;
}

export function livePreview(opts: LivePreviewOptions) {
  const plugin = ViewPlugin.fromClass(
    class {
      deco: DecorationSet;
      atomic: DecorationSet;
      constructor(view: EditorView) { ({ deco: this.deco, atomic: this.atomic } = build(view, opts)); }
      update(u: ViewUpdate) {
        if (u.docChanged || u.selectionSet || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) {
          ({ deco: this.deco, atomic: this.atomic } = build(u.view, opts));
        }
      }
    },
    { decorations: (v) => v.deco, provide: (p) => EditorView.atomicRanges.of((view) => view.plugin(p)?.atomic ?? Decoration.none) },
  );
  const clicks = EditorView.domEventHandlers({
    mousedown: (e, view) => {
      if (!(e.metaKey || e.ctrlKey)) return false;
      const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
      if (pos === null) return false;
      // a Markdown link: [text](address)
      let link: SyntaxNode | null = syntaxTree(view.state).resolveInner(pos, 1);
      while (link && link.name !== 'Link') link = link.parent;
      const url = link?.getChild('URL');
      if (url) {
        e.preventDefault();
        opts.openHref(view.state.sliceDoc(url.from, url.to).replace(/^<|>$/g, ''));
        return true;
      }
      const l = view.state.doc.lineAt(pos);
      for (const m of l.text.matchAll(WIKI)) {
        const a = l.from + m.index!;
        if (pos >= a && pos <= a + m[0].length) {
          e.preventDefault();
          opts.openLink(m[1].split('|')[0].split('#')[0]);
          return true;
        }
      }
      return false;
    },
  });
  return [plugin, EditorState.transactionFilter.of(pairCleanup), clicks, theme];
}

const theme = EditorView.theme({
  '&.cm-formatted .cm-content': { fontFamily: 'var(--serif)', fontSize: '17px', lineHeight: '1.6' },
  '.cm-lp-h1': { fontSize: '1.7em', fontWeight: '700', lineHeight: '1.3', paddingTop: '6px' },
  '.cm-lp-h2': { fontSize: '1.35em', fontWeight: '650', lineHeight: '1.35', paddingTop: '10px' },
  '.cm-lp-h3': { fontSize: '1.15em', fontWeight: '650' },
  '.cm-lp-h4, .cm-lp-h5, .cm-lp-h6': { fontWeight: '650' },
  '&.cm-formatted [class*="cm-lp-h"] span': { textDecoration: 'none' },
  '.cm-lp-strong': { fontWeight: '700' },
  '.cm-lp-em': { fontStyle: 'italic' },
  '.cm-lp-strike, .cm-lp-done': { textDecoration: 'line-through', color: 'var(--muted)' },
  '.cm-lp-code': { fontFamily: 'var(--mono)', fontSize: '0.88em', background: 'var(--sunken)', borderRadius: '4px', padding: '0 3px' },
  '.cm-lp-codeblock': { fontFamily: 'var(--mono)', fontSize: '0.88em', background: 'var(--sunken)' },
  '.cm-lp-quote': { borderLeft: '3px solid var(--accent-soft)', paddingLeft: '14px !important', color: 'var(--muted)' },
  '.cm-lp-fm': { fontFamily: 'var(--mono)', fontSize: '0.8em', color: 'var(--muted)', background: 'var(--sunken)' },
  '.cm-lp-bullet': { color: 'var(--accent)', fontWeight: '700', padding: '0 8px 0 2px' },
  '.cm-lp-listmark': { color: 'var(--accent-ink)' },
  '.cm-lp-task': { margin: '0 8px 0 0', verticalAlign: 'middle', width: '16px', height: '16px', accentColor: 'var(--accent)', cursor: 'pointer' },
  '.cm-lp-link': { color: 'var(--accent-ink)', textDecoration: 'underline', textDecorationColor: 'var(--accent-soft)' },
  '.cm-lp-extmark': { color: 'var(--muted)', fontSize: '0.75em', marginLeft: '1px', verticalAlign: 'top' },
  '.cm-lp-wikilink': { color: 'var(--accent-ink)', textDecoration: 'underline dotted' },
  '.cm-lp-img': { maxWidth: '100%', maxHeight: '320px', borderRadius: '8px', display: 'inline-block', verticalAlign: 'top' },
  '.cm-lp-hr': { display: 'inline-block', width: '100%', borderTop: '1px solid var(--border)', verticalAlign: 'middle' },
});
