// The kitchen whiteboard (server: crates/core/src/board.rs). Reloaded on the change stream's
// `messages` event (state.svelte.ts).
import { api } from './api';

export interface Message { id: string; text: string; author: string; user: string; created: number; expires: number | null }

export const board = $state({ messages: [] as Message[], compose: false });

export async function loadMessages() {
  try { board.messages = await api<Message[]>('GET', '/api/messages'); } catch { /* offline: keep what's shown */ }
}

/** Unix seconds at the end of a local day: today, or a `YYYY-MM-DD` from a date field. */
export function endOfDay(date?: string): number {
  const d = date ? new Date(`${date}T00:00:00`) : new Date();
  d.setHours(23, 59, 59, 0);
  return Math.floor(d.getTime() / 1000);
}

export { today } from './credit';
