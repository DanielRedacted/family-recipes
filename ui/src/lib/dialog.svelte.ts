// Promise-based modal (window.prompt/confirm are unreliable inside the Tauri webview).

export interface DialogReq {
  title: string;
  message?: string;
  value?: string; // present = text input
  options?: string[]; // present = select
  ok?: string;
  /** optional middle button; resolves 'alt' */
  alt?: string;
  danger?: boolean;
  resolve: (v: string | null) => void;
}

export const dialog = $state({ current: null as DialogReq | null });

function open(req: Omit<DialogReq, 'resolve'>): Promise<string | null> {
  return new Promise((resolve) => {
    dialog.current = { ...req, resolve: (v) => { dialog.current = null; resolve(v); } };
  });
}

export const ask = (title: string, value = '', ok = 'OK', message?: string) => open({ title, value, ok, message });
export const choose = (title: string, options: string[], value = '', ok = 'OK') => open({ title, options, value, ok });
export const confirmIt = async (title: string, message?: string, ok = 'OK', danger = false) =>
  (await open({ title, message, ok, danger })) !== null;

/** Three-way choice, e.g. Save / Don't save / Cancel. */
export const confirm3 = async (title: string, message: string, ok: string, alt: string): Promise<'ok' | 'alt' | null> => {
  const r = await open({ title, message, ok, alt });
  return r === 'alt' ? 'alt' : r === null ? null : 'ok';
};
