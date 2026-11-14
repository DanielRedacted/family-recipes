// Formatted ("live preview") editing, Obsidian style: the document stays plain Markdown and is
// saved exactly as typed. Decorations only change how it looks: markup is hidden except on the
// line(s) the cursor is in, headings are sized, bullets become dots, task boxes become
// checkboxes, images show inline, and [[links]] look like links.
import { syntaxTree } from '@codemirror/language';
import { RangeSet, type EditorState, type Range } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from '@codemirror/view';

export interface LivePreviewOptions {
  /** URL for an image reference as written in the note (or null to leave it as text) */
  imageUrl: (src: string) => string | null;
  /** open a [[link]] target (⌘-click) */
  openLink: (target: string) => void;
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

/** Is any selection range on the lines spanned by [from, to]? */
function touches(state: EditorState, from: number, to: number) {
  const a = state.doc.lineAt(from).from;
  const b = state.doc.lineAt(to).to;
  return state.selection.ranges.some((r) => r.to >= a && r.from <= b);
}

function build(view: EditorView, opts: LivePreviewOptions): DecorationSet {
  const { state } = view;
  const out: Range<Decoration>[] = [];
  const doc = state.doc;
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter: (n) => {
        const name = n.name;
        const active = touches(state, n.from, n.to);
        const h = /^(ATX|Setext)Heading(\d)$/.exec(name);
        if (h) { out.push(line(`cm-lp-h${h[2]}`).range(doc.lineAt(n.from).from)); return; }
        switch (name) {
          case 'Frontmatter':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-fm').range(doc.line(l).from));
            return false;
          case 'HeaderMark':
            if (!active && n.to < doc.lineAt(n.from).to) out.push(hide.range(n.from, Math.min(n.to + 1, doc.lineAt(n.from).to)));
            else if (!active) out.push(hide.range(n.from, n.to));
            return;
          case 'EmphasisMark':
          case 'CodeMark':
          case 'StrikethroughMark':
            if (!active && n.node.parent?.name !== 'FencedCode') out.push(hide.range(n.from, n.to));
            return;
          case 'StrongEmphasis': out.push(mark('cm-lp-strong').range(n.from, n.to)); return;
          case 'Emphasis': out.push(mark('cm-lp-em').range(n.from, n.to)); return;
          case 'Strikethrough': out.push(mark('cm-lp-strike').range(n.from, n.to)); return;
          case 'InlineCode': out.push(mark('cm-lp-code').range(n.from, n.to)); return;
          case 'FencedCode':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-codeblock').range(doc.line(l).from));
            return;
          case 'Table':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-table').range(doc.line(l).from));
            return false;
          case 'Blockquote':
            for (let l = doc.lineAt(n.from).number; l <= doc.lineAt(n.to).number; l++) out.push(line('cm-lp-quote').range(doc.line(l).from));
            return;
          case 'QuoteMark':
            if (!active) out.push(hide.range(n.from, Math.min(n.to + (doc.sliceString(n.to, n.to + 1) === ' ' ? 1 : 0), doc.lineAt(n.from).to)));
            return;
          case 'HorizontalRule':
            if (!active) out.push(rule.range(n.from, n.to));
            return;
          case 'ListMark': {
            const text = doc.sliceString(n.from, n.to);
            const task = n.node.parent?.getChild('Task');
            if (/^[-*+]$/.test(text) && !active && !task) out.push(bullet.range(n.from, n.to));
            else if (/^[-*+]$/.test(text) && !active && task) out.push(hide.range(n.from, Math.min(n.to + 1, task.from)));
            else out.push(mark('cm-lp-listmark').range(n.from, n.to));
            return;
          }
          case 'TaskMarker': {
            const checked = /x/i.test(doc.sliceString(n.from, n.to));
            if (!active) out.push(Decoration.replace({ widget: new Checkbox(checked, n.from) }).range(n.from, n.to));
            if (checked) out.push(mark('cm-lp-done').range(n.to, doc.lineAt(n.to).to));
            return;
          }
          case 'Image': {
            const url = n.node.getChild('URL');
            if (!url || active) return;
            const src = opts.imageUrl(doc.sliceString(url.from, url.to).replace(/^<|>$/g, ''));
            const alt = doc.sliceString(n.from + 2, doc.sliceString(n.from, n.to).indexOf(']') + n.from);
            if (src) out.push(Decoration.replace({ widget: new Img(src, alt) }).range(n.from, n.to));
            return false;
          }
          case 'Link': {
            out.push(mark('cm-lp-link').range(n.from, n.to));
            if (active) return false;
            // hide "[" and "](url)" — keep the text
            const marks = n.node.getChildren('LinkMark');
            if (marks.length >= 2) {
              out.push(hide.range(marks[0].from, marks[0].to));
              out.push(hide.range(marks[1].from, n.to));
            }
            return false;
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
      if (!touches(state, a, b)) {
        const bar = m[1].indexOf('|');
        out.push(hide.range(a, a + 2 + (bar >= 0 ? bar + 1 : 0)));
        out.push(hide.range(b - 2, b));
      }
    }
  }
  return RangeSet.of(out, true);
}

export function livePreview(opts: LivePreviewOptions) {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) { this.decorations = build(view, opts); }
      update(u: ViewUpdate) {
        if (u.docChanged || u.selectionSet || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) {
          this.decorations = build(u.view, opts);
        }
      }
    },
    { decorations: (v) => v.decorations },
  );
  const clicks = EditorView.domEventHandlers({
    mousedown: (e, view) => {
      if (!(e.metaKey || e.ctrlKey)) return false;
      const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
      if (pos === null) return false;
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
  return [plugin, clicks, theme];
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
  '.cm-lp-table': { fontFamily: 'var(--mono)', fontSize: '0.85em' },
  '.cm-lp-quote': { borderLeft: '3px solid var(--accent-soft)', paddingLeft: '14px !important', color: 'var(--muted)' },
  '.cm-lp-fm': { fontFamily: 'var(--mono)', fontSize: '0.8em', color: 'var(--muted)', background: 'var(--sunken)' },
  '.cm-lp-bullet': { color: 'var(--accent)', fontWeight: '700', padding: '0 2px' },
  '.cm-lp-listmark': { color: 'var(--accent-ink)' },
  '.cm-lp-task': { margin: '0 6px 0 0', verticalAlign: 'middle', width: '16px', height: '16px', accentColor: 'var(--accent)', cursor: 'pointer' },
  '.cm-lp-link': { color: 'var(--accent-ink)', textDecoration: 'underline', textDecorationColor: 'var(--accent-soft)' },
  '.cm-lp-wikilink': { color: 'var(--accent-ink)', textDecoration: 'underline dotted' },
  '.cm-lp-img': { maxWidth: '100%', maxHeight: '320px', borderRadius: '8px', display: 'inline-block', verticalAlign: 'top' },
  '.cm-lp-hr': { display: 'inline-block', width: '100%', borderTop: '1px solid var(--border)', verticalAlign: 'middle' },
});
