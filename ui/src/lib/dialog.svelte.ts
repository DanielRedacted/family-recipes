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
  placeholder?: string;
  /** a tick box under the text field, and whether it starts ticked */
  check?: string;
  checked?: boolean;
  resolve: (v: string | null, checked?: boolean) => void;
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

/** A text field and a tick box, e.g. a change's comment and "Credit me as an author". */
export const askCheck = (title: string, message: string | undefined, ok: string, check: string, checked: boolean, placeholder?: string) =>
  new Promise<{ text: string; checked: boolean } | null>((resolve) => {
    dialog.current = {
      title, message, ok, check, checked, placeholder, value: '',
      resolve: (v, c) => { dialog.current = null; resolve(v === null ? null : { text: v, checked: !!c }); },
    };
  });

/** Three-way choice, e.g. Save / Don't save / Cancel. */
export const confirm3 = async (title: string, message: string, ok: string, alt: string): Promise<'ok' | 'alt' | null> => {
  const r = await open({ title, message, ok, alt });
  return r === 'alt' ? 'alt' : r === null ? null : 'ok';
};
