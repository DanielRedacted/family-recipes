// Context menu (right-click / ⋯ button).
export interface MenuItem { label: string; run: () => void; danger?: boolean; /** line above this item */ sep?: boolean }
/** from: what opened it, for focus to go back to; kbd: opened from the keyboard, so focus goes into it */
export const menu = $state({ x: 0, y: 0, items: [] as MenuItem[], from: null as HTMLElement | null, kbd: false });

export function openMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  const from = e.currentTarget as HTMLElement | null;
  const r = from?.getBoundingClientRect();
  const fromButton = e.type === 'click' && r;
  menu.x = Math.min(fromButton ? r.left : e.clientX, window.innerWidth - 220);
  menu.y = Math.max(8, Math.min(fromButton ? r.bottom + 4 : e.clientY, window.innerHeight - items.length * 40 - 16));
  menu.from = from;
  menu.kbd = e.type === 'click' && e.detail === 0; // Enter or Space on the ⋯ button
  menu.items = items;
}
/** refocus: put focus back on what opened the menu, so a dialog an item opens returns there too;
 * ring: show the focus ring (closed from the keyboard, not by a click) */
export const closeMenu = (refocus = false, ring = true) => {
  if (refocus) menu.from?.focus({ focusVisible: ring } as FocusOptions);
  menu.items = [];
};
