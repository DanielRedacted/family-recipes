// The ingredient catalogue (built by the server from every recipe) and editor suggestions
// for ingredient lines: units after a quantity, ingredient names after that.
import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { api } from './api';

export interface Ingredient { name: string; key: string; count: number; recipes: string[]; forms: string[] }

export const catalog = $state({ items: [] as Ingredient[], units: [] as string[], loaded: false });

let loading: Promise<void> | null = null;
let loadedAt = 0;
/** Fetch the catalogue (cheap: the server caches it per vault version). */
export function loadIngredients(maxAgeMs = 30_000): Promise<void> {
  if (catalog.loaded && Date.now() - loadedAt < maxAgeMs) return Promise.resolve();
  loading ??= api<{ ingredients: Ingredient[]; units: string[] }>('GET', '/api/ingredients')
    .then((r) => { catalog.items = r.ingredients; catalog.units = r.units; catalog.loaded = true; loadedAt = Date.now(); })
    .catch(() => {})
    .finally(() => { loading = null; });
  return loading;
}

const QTY = /^(?:about\s+)?(?:\d+\s+\d+\/\d+|\d+\/\d+|\d+(?:[.,]\d+)?(?:\s*[½¼¾⅓⅔⅛⅜⅝⅞])?|[½¼¾⅓⅔⅛⅜⅝⅞])(?:\s*(?:-|–|to)\s*(?:\d+\/\d+|\d+(?:[.,]\d+)?|[½¼¾⅓⅔⅛⅜⅝⅞]))?\s+/i;
const UNIT = /^(cups?|c|tablespoons?|tbsps?|tbps|tbs|tbl|tb|teaspoons?|tsps?|ts|fl\.? oz|ounces?|oz|pounds?|lbs?|grams?|g|kg|ml|l|lit(?:er|re)s?|quarts?|qt|pints?|pt|gallons?|gal|pinch(?:es)?|dash(?:es)?|drops?|cloves?|cans?|jars?|packages?|pkgs?|packets?|envelopes?|sticks?|slices?|pieces?|bunch(?:es)?|sprigs?|stalks?|heads?|box(?:es)?|bags?|handfuls?)\.?\s+(?:of\s+)?/i;
const LIST = /^\s*(?:[-*+](?: \[[ xX]\])?|\d+[.)])\s+/;

/** Is the cursor's line an ingredient line? (a list item under an "Ingredients" heading, or before any heading) */
function inIngredients(ctx: CompletionContext, lineFrom: number): boolean {
  const doc = ctx.state.doc;
  for (let n = doc.lineAt(lineFrom).number - 1; n >= 1; n--) {
    const t = doc.line(n).text;
    if (/^#{1,2}\s/.test(t)) return /ingredient|you.?ll need|you will need/i.test(t) || /^#\s/.test(t);
  }
  return true;
}

export function ingredientSuggest(ctx: CompletionContext): CompletionResult | null {
  const line = ctx.state.doc.lineAt(ctx.pos);
  const before = line.text.slice(0, ctx.pos - line.from);
  const marker = LIST.exec(before);
  if (!marker || !inIngredients(ctx, line.from)) return null;
  let rest = before.slice(marker[0].length);
  if (rest.includes('[[')) return null;
  const qty = QTY.exec(rest);
  if (qty) rest = rest.slice(qty[0].length);
  if (!catalog.loaded) { loadIngredients(); return null; }
  const unitMatch = UNIT.exec(rest);
  const hasUnit = !!unitMatch;
  if (unitMatch) rest = rest.slice(unitMatch[0].length);
  if (/[,(]/.test(rest)) return null; // into the notes part
  const typed = rest;
  if (typed.length < 2 && !ctx.explicit && !(qty && !hasUnit && typed.length >= 1)) return null;
  const from = ctx.pos - typed.length;
  const options = [
    ...(qty && !hasUnit ? catalog.units.map((u) => ({ label: u, type: 'keyword', detail: 'unit', boost: 99 })) : []),
    ...catalog.items.map((i) => ({ label: i.name, type: 'variable', detail: `${i.count} recipe${i.count === 1 ? '' : 's'}`, boost: Math.min(i.count, 99) - 100 })),
  ];
  return { from, options, validFor: /^[\p{L}\p{N}' -]*$/u };
}
