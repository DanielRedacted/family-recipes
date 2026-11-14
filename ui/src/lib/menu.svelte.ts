// Context menu (right-click / ⋯ button).
export interface MenuItem { label: string; run: () => void; danger?: boolean; /** line above this item */ sep?: boolean }
export const menu = $state({ x: 0, y: 0, items: [] as MenuItem[] });

export function openMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  const r = (e.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  const fromButton = e.type === 'click' && r;
  menu.x = Math.min(fromButton ? r.left : e.clientX, window.innerWidth - 220);
  menu.y = Math.max(8, Math.min(fromButton ? r.bottom + 4 : e.clientY, window.innerHeight - items.length * 40 - 16));
  menu.items = items;
}
export const closeMenu = () => { menu.items = []; };
