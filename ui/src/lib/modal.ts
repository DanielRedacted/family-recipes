// Native modal <dialog>: the browser keeps focus inside it, makes the page behind inert, closes it on
// Esc (and the back gesture) and puts focus back where it was. Use inside an {#if}:
//   <dialog class="modal-host" use:modal={{ onclose }}>…</dialog>
// - onclose: called whenever the browser closes it, so the owner's state follows.
// - active: false hides it without calling onclose (a modal in a tab that isn't showing keeps its state
//   but must not block the tab that is).
// - outside: a click outside closes it: closedby="any" where supported, a pointer test elsewhere (Safari).
export interface ModalOpts { onclose: () => void; active?: boolean; outside?: boolean }

export function modal(d: HTMLDialogElement, opts: ModalOpts) {
  let o = opts;
  let ours = false; // a close() of our own, which isn't the user closing it
  let back: Element | null = null; // focused before it opened
  const closed = () => { if (ours) ours = false; else o.onclose(); };
  const sync = () => {
    const on = o.active !== false;
    if (on && !d.open) { back = document.activeElement; d.showModal(); }
    else if (!on && d.open) { ours = true; d.close(); }
  };

  // The press and the release must both be outside the box, so a text selection dragged onto the
  // backdrop doesn't close it. The box is tested, not event.target: the dialog is the target for clicks
  // on its own padding too.
  const outside = (e: PointerEvent) => {
    const r = d.getBoundingClientRect();
    return e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom;
  };
  let pressedOutside = false;
  const down = (e: PointerEvent) => { pressedOutside = outside(e); };
  const up = (e: PointerEvent) => { if (o.outside && pressedOutside && outside(e)) d.close(); };
  if (o.outside) d.setAttribute('closedby', 'any');
  const fallback = !('closedBy' in HTMLDialogElement.prototype);
  if (fallback) { d.addEventListener('pointerdown', down); d.addEventListener('pointerup', up); }

  d.addEventListener('close', closed);
  sync();
  return {
    update(next: ModalOpts) { o = next; sync(); },
    destroy() {
      d.removeEventListener('close', closed);
      if (fallback) { d.removeEventListener('pointerdown', down); d.removeEventListener('pointerup', up); }
      if (d.open) d.close();
      // The browser returns focus only when the dialog is closed while still in the page; when its
      // owner removes it (Cancel, Save), focus has fallen to <body>, so put it back by hand.
      if (document.activeElement === document.body && back instanceof HTMLElement && back.isConnected) back.focus();
    },
  };
}
