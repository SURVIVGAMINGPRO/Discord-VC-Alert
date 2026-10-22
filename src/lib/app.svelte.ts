// Shared reactive state for a window: the latest snapshot from Rust, the
// profile pictures, and toasts.
import { call, getState, on } from './api';
import { ms } from './motion';
import type { Snapshot } from './types';

export const app = $state<{ snap: Snapshot | null; label: string }>({ snap: null, label: 'main' });

export async function initState(label: string) {
  app.label = label;
  app.snap = await getState();
  await on<Snapshot>('state', (s) => (app.snap = s));
}

/**
 * Saved theme values map onto the token themes: "onyx" is Ink, "graphite" is
 * Graphite, "paper" is Paper, "night" falls back to Ink, and "system" picks
 * Ink or Paper from the OS setting.
 */
export function resolveTheme(theme: string): 'ink' | 'graphite' | 'paper' {
  if (theme === 'graphite') return 'graphite';
  if (theme === 'paper') return 'paper';
  if (theme === 'system') return matchMedia('(prefers-color-scheme: dark)').matches ? 'ink' : 'paper';
  return 'ink';
}

export function applyAppearance(theme: string, density: string) {
  document.documentElement.dataset.theme = resolveTheme(theme);
  document.documentElement.dataset.density = density;
}

// ------------------------------------------------------------ pictures --

export const avatars = $state<Record<string, string>>({});

export async function loadAvatars(keys: string[]) {
  const missing = keys.filter((k) => !(k in avatars));
  if (!missing.length) return;
  const got = await call<Record<string, string>>('avatar_data', { keys: missing });
  Object.assign(avatars, got);
}

export async function watchAvatars() {
  await on<string>('avatar', async (key) => {
    const got = await call<Record<string, string>>('avatar_data', { keys: [key] });
    Object.assign(avatars, got);
  });
}

// --------------------------------------------------------------- toasts --

export interface Toast {
  id: number;
  kind: 'ok' | 'error' | 'info';
  title: string;
  /** A muted second line. */
  detail?: string;
  action?: { label: string; run: () => void };
  /** How long it stays, in ms (paused while you hover the stack). */
  ms: number;
  leaving?: boolean;
}

/** Newest first. Up to TOAST_LIMIT stay on screen; older ones leave. */
export const toasts = $state<{ list: Toast[] }>({ list: [] });
export const TOAST_LIMIT = 3;
let seq = 0;

/** "Restored. Your list is back." → title "Restored", detail "Your list is back." */
function split(message: string): [string, string | undefined] {
  const m = /^(.+?)\.\s+(?=[A-Z“"(])(.+)$/s.exec(message.trim());
  if (m) return [m[1], m[2]];
  return [message.trim().replace(/\.$/, ''), undefined];
}

export function toast(
  kind: Toast['kind'],
  message: string,
  opts: { detail?: string; action?: Toast['action']; ms?: number } = {},
) {
  const [title, detail] = opts.detail ? [message, opts.detail] : split(message);
  const t: Toast = { id: ++seq, kind, title, detail, action: opts.action, ms: opts.ms ?? 5000 };
  toasts.list = [t, ...toasts.list];
  toasts.list.filter((x) => !x.leaving).slice(TOAST_LIMIT).forEach((x) => hideToast(x.id));
}

/** Plays the toast's exit (in place, --t-dismiss), then removes it. */
export function hideToast(id: number) {
  const t = toasts.list.find((x) => x.id === id);
  if (!t || t.leaving) return;
  t.leaving = true;
  setTimeout(() => (toasts.list = toasts.list.filter((x) => x.id !== id)), ms('--t-dismiss'));
}
