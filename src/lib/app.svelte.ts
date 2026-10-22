// Shared reactive state for a window: the latest snapshot from Rust, the
// profile pictures, and toasts.
import { call, getState, on } from './api';
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
  message: string;
  action?: { label: string; run: () => void };
}

export const toasts = $state<{ current: Toast | null }>({ current: null });
let timer: ReturnType<typeof setTimeout> | undefined;
let seq = 0;

/** A new toast replaces the old one (v1 behaviour). */
export function toast(kind: Toast['kind'], message: string, opts: { action?: Toast['action']; ms?: number } = {}) {
  clearTimeout(timer);
  toasts.current = { id: ++seq, kind, message, action: opts.action };
  timer = setTimeout(() => (toasts.current = null), opts.ms ?? 5000);
}

export function hideToast() {
  clearTimeout(timer);
  toasts.current = null;
}
