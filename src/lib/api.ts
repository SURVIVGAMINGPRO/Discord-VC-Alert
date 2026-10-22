// Talks to the Rust side. In a plain browser (`npm run dev` without Tauri) it
// falls back to a small in-memory mock so the UI can be previewed and tested.

import type { Snapshot } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

type Handler = (payload: any) => void;

export async function call<T = void>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  const { mockInvoke } = await import('./mock');
  return mockInvoke(cmd, args) as Promise<T>;
}

export async function on<T = any>(event: string, cb: (payload: T) => void): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(event, (e) => cb(e.payload));
  }
  const { mockListen } = await import('./mock');
  return mockListen(event, cb as Handler);
}

/** Which window this page is: main, settings, overlay, editor-ghost, editor-panel. */
export async function windowLabel(): Promise<string> {
  if (inTauri) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow().label;
  }
  return new URLSearchParams(location.search).get('view') ?? 'main';
}

export function getState(): Promise<Snapshot> {
  return call<Snapshot>('get_state');
}

export function errText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) return String((e as Error).message);
  return String(e);
}

export async function pickFile(title: string, filters: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ title, multiple: false, directory: false, filters });
  return typeof r === 'string' ? r : null;
}

export async function saveFile(title: string, defaultPath: string): Promise<string | null> {
  if (!inTauri) return null;
  const { save } = await import('@tauri-apps/plugin-dialog');
  return save({ title, defaultPath, filters: [{ name: 'JSON', extensions: ['json'] }] });
}

export async function closeWindow(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().close();
}

/** Files dragged onto the window (Tauri only). */
export async function onFileDrop(cb: (kind: 'over' | 'drop' | 'leave', paths: string[]) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  return getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    if (p.type === 'over' || p.type === 'enter') cb('over', 'paths' in p ? p.paths : []);
    else if (p.type === 'drop') cb('drop', p.paths);
    else cb('leave', []);
  });
}
