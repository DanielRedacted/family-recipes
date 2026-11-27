import { describe, it, expect } from 'vitest';
import { volumeRows, weightRows, ovenRows, SAFE_TEMPS, convert, VOLUME_COLS } from './charts';

const row = (label: string) => {
  const r = volumeRows().find((x) => x.label === label)!;
  return Object.fromEntries(VOLUME_COLS.map((c, i) => [c, r.cells[i]]));
};

describe('kitchen charts', () => {
  it('measure equivalents', () => {
    expect(row('⅓ cup')).toMatchObject({ tsp: '16', tbsp: '5⅓', 'fl oz': '2⅔', ml: '79' });
    expect(row('⅔ cup')).toMatchObject({ tbsp: '10⅔' });
    expect(row('1 cup')).toMatchObject({ tsp: '48', tbsp: '16', cup: '1', 'fl oz': '8', ml: '237' });
    expect(row('1 tsp')).toMatchObject({ tbsp: '⅓', cup: '', 'fl oz': '' });
    expect(row('1 gallon')).toMatchObject({ tsp: '', tbsp: '', cup: '16', 'fl oz': '128' });
    expect(weightRows().find((r) => r.label === '1 lb')!.cells).toEqual(['16', '1', '454']);
  });
  it('temperatures round as the recipes do', () => {
    expect(ovenRows().find((r) => r.f === 350)!.c).toBe(175);
    expect(SAFE_TEMPS.find((r) => r.f === 165)!.c).toBe(74);
    expect(SAFE_TEMPS.find((r) => r.f === 145)!.c).toBe(63);
  });
  it('converter', () => {
    expect(convert('1 ½', 'cup')).toContainEqual({ unit: 'tbsp', value: '24' });
    expect(convert('1/3', 'cup')).toContainEqual({ unit: 'tsp', value: '16' });
    expect(convert('⅓', 'cup')).toContainEqual({ unit: 'ml', value: '79' });
    expect(convert('100', 'g')).toContainEqual({ unit: 'oz', value: '3½', about: true });
    expect(convert('1', 'tsp').map((r) => r.unit)).not.toContain('gal');
    expect(convert('⅓', 'cup').map((r) => r.unit)).not.toContain('pt'); // ⅙ pt, not "about ⅛"
    expect(convert('350', '°F')).toEqual([{ unit: '°C', value: '175' }]);
    expect(convert('180', '°C')).toEqual([{ unit: '°F', value: '356' }]);
    expect(convert('abc', 'cup')).toEqual([]);
    expect(convert('', 'cup')).toEqual([]);
  });
});
