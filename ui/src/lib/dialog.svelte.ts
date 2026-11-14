// Promise-based modal (window.prompt/confirm are unreliable inside the Tauri webview).

export interface DialogReq {
  title: string;
  message?: string;
  value?: string; // present = text input
  options?: string[]; // present = select
  ok?: string;
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
