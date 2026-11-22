import { describe, expect, it } from 'vitest';
import { pages, helpPages, searchHelp } from './help';

describe('help pages', () => {
  it('every page has its frontmatter', () => {
    expect(pages.length).toBeGreaterThan(5);
    for (const p of pages) {
      expect(p.title, p.slug).toBeTruthy();
      expect(p.summary, p.slug).toBeTruthy();
      expect(['everyone', 'admin'], p.slug).toContain(p.audience);
      expect(p.order, p.slug).not.toBe(999);
      expect(p.body.trimStart(), p.slug).not.toMatch(/^# /); // the page title is the heading
    }
  });

  it('links between pages point at real pages', () => {
    const slugs = new Set(pages.map((p) => p.slug));
    for (const p of pages)
      for (const m of p.body.matchAll(/#\/settings\/help\/([\w-]+)/g)) expect(slugs, `${p.slug} -> ${m[1]}`).toContain(m[1]);
  });

  it('pages for everyone never link to admin pages', () => {
    const admin = new Set(pages.filter((p) => p.audience === 'admin').map((p) => p.slug));
    for (const p of helpPages(false))
      for (const m of p.body.matchAll(/#\/settings\/help\/([\w-]+)/g)) expect(admin.has(m[1]), `${p.slug} -> ${m[1]}`).toBe(false);
  });

  it('admin pages are hidden from other people, in the list and in search', () => {
    expect(helpPages(false).every((p) => p.audience === 'everyone')).toBe(true);
    expect(helpPages(true).length).toBe(pages.length);
    expect(searchHelp('frontmatter', true).some((p) => p.slug === 'file-format')).toBe(true);
    expect(searchHelp('frontmatter', false).some((p) => p.audience === 'admin')).toBe(false);
  });
});
