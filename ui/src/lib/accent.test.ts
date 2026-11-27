import { describe, expect, it } from 'vitest';
import { accentTokens, accentCss, contrast, mix, SWATCHES } from './accent';

describe('accent colour', () => {
  it('mixes in OKLab and measures contrast like WCAG', () => {
    expect(mix('#7a52cc', '#000000', 0)).toBe('#7a52cc');
    expect(mix('#7a52cc', '#ffffff', 1)).toBe('#ffffff');
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21);
  });

  it('keeps the built-in purple close to the hand-picked tokens', () => {
    const t = accentTokens('#7a52cc');
    expect(t.light['--accent']).toBe('#7a52cc');
    for (const [got, want] of [[t.light['--accent-ink'], '#5e3ba8'], [t.light['--accent-soft'], '#ebe3fb'], [t.dark['--accent'], '#b39cf2'], [t.dark['--accent-soft'], '#2d2442']])
      expect(contrast(got, want), `${got} ≈ ${want}`).toBeLessThan(1.25);
  });

  it('keeps any colour readable as text and under its button label', () => {
    for (const c of [...SWATCHES.map((s) => s.value), '#ffee00', '#ffffff', '#000000', '#00ff00']) {
      const { light, dark } = accentTokens(c);
      expect(contrast(light['--accent'], '#faf9fd'), c).toBeGreaterThanOrEqual(4.5);
      expect(contrast(dark['--accent'], '#141319'), c).toBeGreaterThanOrEqual(4.5);
      expect(contrast(light['--accent'], light['--on-accent']), c).toBeGreaterThanOrEqual(4.5);
      expect(contrast(dark['--accent'], dark['--on-accent']), c).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('writes CSS only for a real colour', () => {
    expect(accentCss(null)).toBe('');
    expect(accentCss('red;}body{display:none')).toBe('');
    expect(accentCss('#2F6FD0')).toMatch(/^html:root\{--accent:#[0-9a-f]{6};.*\[data-theme="dark"\]/);
    expect(accentCss('#2f6fd0', true)).not.toContain('dark');
  });
});
