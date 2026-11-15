// Write every measurement unit in a recipes folder one way (ui/src/lib/units.ts):
//   node scripts/normalize-units.ts ~/Recipes [--dry]
// Prints each changed line. Tables whose cells changed are re-padded in the editor's layout, so
// opening them in the app changes nothing. Folders starting with "." (.git, .tools, .trash) are skipped.
import fs from 'node:fs';
import path from 'node:path';
import { normalizeUnits } from '../ui/src/lib/units.ts';

const [root, flag] = process.argv.slice(2);
if (!root) { console.error('usage: node scripts/normalize-units.ts <recipes folder> [--dry]'); process.exit(1); }
const dry = flag === '--dry';

const jsLen = (s: string) => s.length; // UTF-16 code units, as the editor counts them
const cells = (line: string) => {
  const parts = line.trim().split(/(?<!\\)\|/);
  if (parts.length && !parts[0].trim()) parts.shift();
  if (parts.length && !parts[parts.length - 1].trim()) parts.pop();
  return parts.map((c) => c.trim());
};
/** Rows in the padded layout the formatted editor writes (each column as wide as its widest cell). */
function padTable(lines: string[]): string[] {
  const rows = lines.filter((_, i) => i !== 1).map(cells);
  const n = Math.max(...rows.map((r) => r.length));
  const full = rows.map((r) => [...r, ...Array(n - r.length).fill('')]);
  const w = Array.from({ length: n }, (_, c) => Math.max(1, ...full.map((r) => jsLen(r[c]))));
  const row = (r: string[]) => '|' + r.map((c, i) => ` ${c}${' '.repeat(w[i] - jsLen(c))} |`).join('');
  return [row(full[0]), '|' + w.map((x) => ` ${'-'.repeat(x)} |`).join(''), ...full.slice(1).map(row)];
}
/** Re-pad tables in `after` whose text differs from `before` (same line count: units change in place). */
function repad(before: string[], after: string[]): string[] {
  const out = [...after];
  for (let i = 0; i < out.length; i++) {
    if (!out[i].trimStart().startsWith('|')) continue;
    let j = i;
    while (j < out.length && out[j].trimStart().startsWith('|')) j++;
    const changed = out.slice(i, j).some((l, k) => l !== before[i + k]);
    const sep = j - i >= 2 && /^\|?(\s*:?-+:?\s*\|)+\s*$/.test(out[i + 1].trim());
    const regular = new Set(out.slice(i, j).filter((_, k) => k !== 1).map((l) => cells(l).length)).size === 1;
    if (changed && sep && regular) out.splice(i, j - i, ...padTable(out.slice(i, j)));
    i = j;
  }
  return out;
}

let files = 0, lines = 0;
const walk = (dir: string): string[] => fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
  e.name.startsWith('.') ? [] : e.isDirectory() ? walk(path.join(dir, e.name)) : e.name.endsWith('.md') ? [path.join(dir, e.name)] : []);
for (const file of walk(root).sort()) {
  const text = fs.readFileSync(file, 'utf8');
  const before = text.split('\n');
  const after = repad(before, normalizeUnits(text).split('\n'));
  const next = after.join('\n');
  if (next === text) continue;
  files++;
  console.log(`\n== ${path.relative(root, file)}`);
  after.forEach((l, i) => {
    if (l !== before[i]) { lines++; console.log(`  - ${before[i]}\n  + ${l}`); }
  });
  if (!dry) fs.writeFileSync(file, next);
}
console.log(`\n${dry ? 'would change' : 'changed'} ${lines} lines in ${files} files`);
