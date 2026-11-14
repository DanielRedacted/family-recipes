import { describe, expect, it } from 'vitest';
import { formatNum, parseNum, scaleLine, yieldNumber } from './scale';

describe('scale', () => {
  it('parses quantity forms', () => {
    expect(parseNum('1 1/2')).toBe(1.5);
    expect(parseNum('1½')).toBe(1.5);
    expect(parseNum('1 ½')).toBe(1.5);
    expect(parseNum('¾')).toBe(0.75);
    expect(parseNum('1⁄4')).toBe(0.25);
    expect(parseNum('2.5')).toBe(2.5);
  });
  it('formats kitchen fractions', () => {
    expect(formatNum(1.5)).toBe('1½');
    expect(formatNum(1 / 3)).toBe('⅓');
    expect(formatNum(2)).toBe('2');
    expect(formatNum(0.99)).toBe('1');
  });
  it('scales lines as written', () => {
    expect(scaleLine('1 1/4 cups sugar', 2)).toBe('2½ cups sugar');
    expect(scaleLine('½ cup sugar', 2)).toBe('1 cup sugar');
    expect(scaleLine('1-2 cups milk', 2)).toBe('2–4 cups milk');
    expect(scaleLine('1½ tsp salt', 1 / 3)).toBe('½ tsp salt');
    expect(scaleLine('1 (10.75 ounce) can soup', 2)).toBe('2 (10.75 ounce) can soup');
    expect(scaleLine('salt to taste', 2)).toBe('salt to taste');
    expect(scaleLine('3 large eggs', 1)).toBe('3 large eggs');
  });
  it('converts units', () => {
    expect(scaleLine('1 cup milk', 1, 'metric')).toBe('237 ml milk');
    expect(scaleLine('2 lb beef', 1, 'metric')).toBe('907 g beef');
    expect(scaleLine('3 tsp salt', 1, 'us')).toBe('1 tbsp salt');
    expect(scaleLine('3 large eggs', 2, 'metric')).toBe('6 large eggs');
  });
  it('reads yields', () => {
    expect(yieldNumber('6 servings')).toBe(6);
    expect(yieldNumber('12-15')).toBe(12);
    expect(yieldNumber(8)).toBe(8);
    expect(yieldNumber('about 3 ½ pounds')).toBe(3.5);
    expect(yieldNumber('Makes 30 dumplings')).toBe(30);
    expect(yieldNumber('to taste')).toBeNull();
  });
});
