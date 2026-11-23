import { describe, expect, it } from 'vitest';
import { dailyValue, estimate, gramsOf, ingredientList, labelAmount, type Food, type Foods } from './nutrition';
import type { Amount, Entry, Group } from './types';

// Foods with round numbers, so totals can be checked by hand (per 100 g).
const foods: Foods = {
  flour: { fdc: 1, name: 'Flour', n: { calories: 400, carbohydrates: 80 }, ml: 0.5 },
  egg: { fdc: 2, name: 'Egg', n: { calories: 150, fat: 10 }, ml: 1, g: { large: 50, medium: 44, each: 50 } },
  oil: { fdc: 3, name: 'Oil', n: { calories: 900, fat: 100 }, ml: 0.9 },
  salt: { fdc: 4, name: 'Salt', n: { sodium: 40000 }, ml: 1.2 },
  garlic: { fdc: 5, name: 'Garlic', n: { calories: 150 }, g: { clove: 3 } },
  lard: null,
};

const line = (text: string, a: Amount | null, i = 0) => ({ line: i, text, amounts: [a] });
const note = (path: string, groups: Group[], frontmatter: Record<string, unknown> = {}): Entry =>
  ({ path, kind: 'note', size: 0, mtime: 0, etag: '', title: path.replace(/\.md$/, ''), body_line: 0, frontmatter, recipe: { ingredients: groups, steps: [] } });
const list = (...items: ReturnType<typeof line>[]): Group => ({ name: null, items });

const bread = note('Bread.md', [list(
  line('2 cups flour', { qty: 2, unit: 'cup', key: 'flour' }),
  line('2 large eggs', { qty: 2, size: 'large', key: 'egg' }),
  line('1 tsp salt', { qty: 1, unit: 'tsp', key: 'salt' }),
  line('Salt to taste', { key: 'salt' }),
  line('Lard for frying', { key: 'lard', skip: 'for frying' }),
  line('Mix:', null),
)]);
const none = () => null;

describe('nutrition', () => {
  it('weighs a line by weight, volume, size and count', () => {
    const f = foods.flour as Food;
    expect(gramsOf({ qty: 2, unit: 'cup' }, f)).toBeCloseTo(2 * 236.588 * 0.5);
    expect(gramsOf({ qty: 4, unit: 'oz' }, f)).toBeCloseTo(4 * 28.3495);
    expect(gramsOf({ qty: 3, size: 'medium' }, foods.egg as Food)).toBe(132);
    expect(gramsOf({ qty: 3 }, foods.egg as Food)).toBe(150);
    expect(gramsOf({ qty: 2, unit: 'clove' }, foods.garlic as Food)).toBe(6);
    expect(gramsOf({ qty: 1.5, unit: 'cup', grams: 150 }, f)).toBe(150); // a weight written on the line wins
    expect(gramsOf({ qty: 1, unit: 'cup' }, foods.garlic as Food)).toBe('no weight for a cup');
    expect(gramsOf({ qty: 1 }, f)).toBe('no unit'); // "¾ milk"
    expect(gramsOf({}, f)).toBe('no amount');
  });

  it('adds up a recipe and lists what it leaves out', () => {
    const r = estimate(bread, foods, none);
    const flour = 2 * 236.588 * 0.5, salt = 4.929 * 1.2;
    expect(r.totals.calories).toBeCloseTo(flour * 4 + 100 * 1.5);
    expect(r.totals.sodium).toBeCloseTo(salt * 400);
    expect(r.totals.fat).toBeCloseTo(10);
    expect([r.counted, r.lines]).toEqual([3, 5]); // "Mix:" is not an ingredient
    expect(r.skipped).toEqual([{ text: 'Salt to taste', why: 'no amount' }, { text: 'Lard for frying', why: 'for frying' }]);
  });

  it('follows the scaler', () => {
    expect(estimate(bread, foods, none, 2).totals.calories).toBeCloseTo(2 * estimate(bread, foods, none).totals.calories);
  });

  it("counts a table's chosen portion", () => {
    const pie = note('Pie.md', [{ name: null, portions: ['Single', 'Double'], items: [
      { line: 1, text: '1 cup flour', alts: ['1 cup flour', '2 cups flour'], amounts: [{ qty: 1, unit: 'cup', key: 'flour' }, { qty: 2, unit: 'cup', key: 'flour' }] },
      { line: 2, text: '1 egg', alts: ['1 egg', '2 egg'], amounts: [{ qty: 1, key: 'egg' }, { qty: 2, key: 'egg' }] },
    ] }]);
    const single = estimate(pie, foods, none, 1, 0).totals.calories;
    // the Double chip (scaler ×2) shows the Double column as written, as the reader does
    expect(estimate(pie, foods, none, 2, 1).totals.calories).toBeCloseTo(2 * single);
    expect(estimate(pie, foods, none, 2, 0).totals.calories).toBeCloseTo(2 * single);
    expect(estimate(pie, foods, none, 1.5, 1).totals.calories).toBeCloseTo(1.5 * single); // Double scaled to ×1½
  });

  describe('an ingredient that is a recipe of its own', () => {
    // 2 eggs (100 g, ~97 ml) + 1 cup oil (~213 g, 236.588 ml)
    const mayo = note('Mayonnaise.md', [list(
      line('2 eggs', { qty: 2, key: 'egg' }),
      line('1 cup oil', { qty: 1, unit: 'cup', key: 'oil' }),
      line('1 3/4 Oil blend', { qty: 1.75, key: 'oil' }),
    )]);
    const whole = estimate(mayo, foods, none);
    const books: Record<string, Entry> = { mayonnaise: mayo };
    const sub = (text: string) => books[Object.keys(books).find((k) => text.toLowerCase().includes(k)) ?? ''] ?? null;

    it('by volume, as a share of what it makes', () => {
      const r = estimate(note('Slaw.md', [list(line('½ cup mayonnaise', { qty: 0.5, unit: 'cup', key: 'mayonnaise' }))]), foods, sub);
      expect(r.totals.calories).toBeCloseTo(whole.totals.calories * (0.5 * 236.588) / whole.ml);
      expect(r.counted).toBe(1);
      // what the sub-recipe couldn't count is listed on the recipe that uses it
      expect(r.skipped).toEqual([{ text: 'Mayonnaise: 1 3/4 Oil blend', why: 'no unit' }]);
    });
    it('by weight', () => {
      const r = estimate(note('Dip.md', [list(line('100 g mayonnaise', { qty: 100, unit: 'g', key: 'mayonnaise' }))]), foods, sub);
      expect(r.totals.calories).toBeCloseTo(whole.totals.calories * 100 / whole.grams);
    });
    it('by count: batches, or a share of its yield', () => {
      const r = estimate(note('Cake.md', [list(line('2 jars mayonnaise', { qty: 2, unit: 'jar', key: 'mayonnaise' }))]), foods, sub);
      expect(r.totals.calories).toBeCloseTo(2 * whole.totals.calories);
      books.mayonnaise = { ...mayo, frontmatter: { yield: '4 jars' } };
      const q = estimate(note('Cake.md', [list(line('1 jar mayonnaise', { qty: 1, unit: 'jar', key: 'mayonnaise' }))]), foods, sub);
      expect(q.totals.calories).toBeCloseTo(whole.totals.calories / 4);
      books.mayonnaise = mayo;
    });
    it('nested two deep, and never through itself', () => {
      const dressing = note('Dressing.md', [list(line('1 cup mayonnaise', { qty: 1, unit: 'cup', key: 'mayonnaise' }), line('1 tsp salt', { qty: 1, unit: 'tsp', key: 'salt' }))]);
      books.dressing = dressing;
      const salad = estimate(note('Salad.md', [list(line('¼ cup dressing', { qty: 0.25, unit: 'cup', key: 'dressing' }))]), foods, sub);
      const d = estimate(dressing, foods, sub);
      expect(salad.totals.calories).toBeCloseTo(d.totals.calories * (0.25 * 236.588) / d.ml);
      expect(salad.totals.calories).toBeGreaterThan(0);
      // a recipe that names itself (through another) is not counted round in circles
      books.mayonnaise = note('Mayonnaise.md', [list(line('1 tbsp dressing', { qty: 1, unit: 'tbsp', key: 'dressing' }), line('1 cup oil', { qty: 1, unit: 'cup', key: 'oil' }))]);
      const loop = estimate(dressing, foods, sub);
      expect(loop.skipped.some((s) => /uses this recipe/.test(s.why))).toBe(true);
      books.mayonnaise = mayo;
      delete books.dressing;
    });
    it('lists the ingredients like a food label: by weight, each once, a recipe with its own in brackets', () => {
      const slaw = estimate(note('Slaw.md', [list(
        line('Salt to taste', { key: 'salt' }),
        line('1 tbsp oil', { qty: 1, unit: 'tbsp', key: 'oil' }),
        line('½ cup mayonnaise', { qty: 0.5, unit: 'cup', key: 'mayonnaise' }), // about 110 g
        line('1 cup flour', { qty: 1, unit: 'cup', key: 'flour' }),
        line('1 cup flour', { qty: 1, unit: 'cup', key: 'flour' }), // 236 g in all
      )]), foods, sub);
      expect(ingredientList(slaw.parts, (k) => k)).toEqual({ text: 'flour, Mayonnaise (oil, egg), oil, salt', unweighed: 1 });
    });
  });

  it('rounds like a Nutrition Facts label', () => {
    expect(labelAmount('calories', 3)).toBe('0');
    expect(labelAmount('calories', 47)).toBe('45');
    expect(labelAmount('calories', 476)).toBe('480');
    expect(labelAmount('fat', 0.4)).toBe('0');
    expect(labelAmount('fat', 2.3)).toBe('2.5');
    expect(labelAmount('fat', 26.4)).toBe('26');
    expect(labelAmount('cholesterol', 3)).toBe('less than 5');
    expect(labelAmount('cholesterol', 66)).toBe('65');
    expect(labelAmount('sodium', 323)).toBe('320');
    expect(labelAmount('sodium', 72)).toBe('70');
    expect(labelAmount('protein', 0.7)).toBe('less than 1');
    expect(labelAmount('protein', 6.4)).toBe('6');
    expect(labelAmount('calcium', 52)).toBe('50');
    expect(dailyValue('sodium', 2300)).toBe(100);
    expect(dailyValue('protein', 10)).toBeNull();
  });
});
