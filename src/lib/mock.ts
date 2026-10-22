// In-browser stand-in for the Rust backend, used only when the UI runs outside
// Tauri (UI preview and tests). It mimics the real commands closely enough to
// click through every screen.

import type { Config, Member, Preview, Snapshot, Target } from './types';
import { keyOf, parseIdent } from './util';

const listeners = new Map<string, Set<(p: any) => void>>();
const qp = new URLSearchParams(location.search);

const people: Target[] = [
  { label: 'Moderator', username: 'moderator' },
  { label: 'Maria', username: 'maria_k', sound: { beep: { frequency_hz: 1400, repeats: 2 } } },
  { label: 'The Twins', id: '123456789012345678', sound: { beep: { frequency_hz: 650, duration_ms: 100 } } },
  { label: 'pixel_fox', username: 'pixel_fox' },
  { label: 'zed_42', username: 'zed_42' },
  { label: 'night.owl', username: 'night.owl' },
  { label: 'Sam', username: 'samwise' },
];

const config: Config = {
  client_id: '1234567890123456789',
  redirect_uri: 'http://localhost',
  scopes: ['rpc', 'rpc.voice.read'],
  targets: qp.get('empty') === '1' ? [] : people,
  alert_if_already_present: true,
  cooldown_seconds: 5,
  sound_file: '',
  beep: { frequency_hz: 1700, duration_ms: 90, repeats: 3, gap_ratio: 0.5, volume: 0.75 },
  beep_profiles: qp.get('profiles') === '3'
    ? [
        { name: 'Low hum', beep: { frequency_hz: 400, duration_ms: 300, repeats: 2, volume: 0.6 } },
        { name: 'Tick', beep: { frequency_hz: 1900, duration_ms: 50, repeats: 1, volume: 0.7 } },
        { name: 'Night mode', beep: { frequency_hz: 900, duration_ms: 120, repeats: 2, volume: 0.35 } },
      ]
    : [{ name: 'Low hum', beep: { frequency_hz: 400, duration_ms: 300, repeats: 2, volume: 0.6 } }],
  overlay: {
    enabled: qp.get('overlay') !== 'off', corner: 'top-left', offset: [24, 24], avatar_px: Number(qp.get('px') ?? 24), layout: 'row', max_shown: 3,
    opacity: Number(qp.get('opacity') ?? 1), bubble: 'both', hide_when_empty: true, style: (qp.get('style') as any) ?? 'pictures', display: 'primary', hide_from_capture: true,
  },
  general: {
    start_watching_on_launch: false, start_minimized: false, mask_usernames: qp.get('mask') === '1', quiet: qp.get('quiet') === '1',
    close_to_tray: true, launch_at_login: false, hide_dock_icon: false,
    setup_complete: qp.get('wizard') !== '1',
  },
  appearance: { theme: (qp.get('theme') as any) ?? 'onyx', density: (qp.get('density') as any) ?? 'compact' },
};

const snap: Snapshot = {
  config,
  has_secret: true,
  watching: false,
  stopping: false,
  status: 'stopped',
  detail: '',
  members: [],
  activity: [],
  platform: navigator.userAgent.includes('Mac') ? 'macos' : 'windows',
  version: '2.0.2',
  data_dir: '~/Library/Application Support/VC Alert',
  displays: [{ name: 'Built-in Display', label: 'Built-in Display (1512 × 982)', primary: true }],
  notice: null,
  editor_open: false,
};

// ?status=… previews each header state without a real Discord.
const STATES: Record<string, Partial<Snapshot>> = {
  connecting: { watching: true, status: 'connecting' },
  idle: { watching: true, status: 'idle_no_channel' },
  watching: { watching: true, status: 'watching_channel', detail: 'General' },
  watching0: { watching: true, status: 'watching_channel', detail: 'The very long channel name for late night strategy talk' },
  reconnecting: { watching: true, status: 'reconnecting', detail: 'Could not reach the Discord desktop app. Is it running? (The browser version of Discord does not work.)' },
  reconnecting2: { watching: true, status: 'reconnecting', detail: 'Connection to Discord failed: broken pipe (os error 32) while reading the next frame' },
  setup_error: { watching: false, status: 'setup_error', detail: "Discord refused the authorization request (RPC error 4002: invalid client). Check the Application ID, and that your account is on the app's App Testers list." },
};
const preset = STATES[qp.get('status') ?? ''];
if (preset) Object.assign(snap, preset);
if (qp.get('status') === 'watching') snap.members = [member(people[1]), member(people[0]), member(people[3])];
if (qp.get('notice') === '1') snap.notice = 'settings.json is not valid JSON. It was moved aside and VC Alert started fresh.';
if (qp.get('nocreds') === '1') snap.has_secret = false;

let removed: { id: number; t: Target; i: number }[] = [];
let nextId = 1;

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb(structuredClone(payload)));
}
function changed() {
  emit('state', snap);
}

function member(t: Target): Member {
  return { key: keyOf(t), id: t.id ?? '1', username: t.username ?? '', avatar: null };
}

export function mockListen(event: string, cb: (p: any) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)!.delete(cb);
}

function deepMerge(target: any, patch: any) {
  for (const [k, v] of Object.entries(patch)) {
    if (v && typeof v === 'object' && !Array.isArray(v) && typeof target[k] === 'object') deepMerge(target[k], v);
    else target[k] = v;
  }
}

export async function mockInvoke(cmd: string, a: any): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 30));
  const find = (key: string) => config.targets.findIndex((t) => keyOf(t).toLowerCase() === key.replace(/^@/, '').toLowerCase());
  switch (cmd) {
    case 'get_state':
      return structuredClone(snap);
    case 'avatar_data':
      return {};
    case 'add_target': {
      const p = parseIdent(a.ident);
      if (!a.ident.trim()) throw 'Enter a username or user ID.';
      if (!p) throw 'Enter a Discord username (letters, numbers, _ and . only; this is the unique username, not the display name) or a numeric user ID.';
      const key = p.id ?? p.username!;
      if (find(key) >= 0) throw 'That person is already on the list.';
      const t: Target = { label: a.label.trim() || key, ...p };
      config.targets.push(t);
      changed();
      return t;
    }
    case 'add_many': {
      const added: Target[] = [];
      const skipped: { value: string; reason: string }[] = [];
      for (const part of String(a.text).split(/[\s,;]+/).filter(Boolean)) {
        const p = parseIdent(part);
        if (!p) skipped.push({ value: part, reason: 'not a valid username or user ID' });
        else if (find(p.id ?? p.username!) >= 0) skipped.push({ value: part, reason: 'already on your list' });
        else {
          const t = { label: p.id ?? p.username!, ...p };
          config.targets.push(t);
          added.push(t);
        }
      }
      changed();
      return { added, skipped };
    }
    case 'remove_target': {
      const i = find(a.key);
      if (i < 0) return null;
      const [t] = config.targets.splice(i, 1);
      const id = nextId++;
      removed.push({ id, t, i });
      changed();
      return { id, label: t.label };
    }
    case 'undo_remove': {
      const r = removed.find((x) => x.id === a.id);
      if (r) {
        config.targets.splice(r.i, 0, r.t);
        removed = removed.filter((x) => x !== r);
        changed();
      }
      return;
    }
    case 'rename_target': {
      const i = find(a.key);
      if (i >= 0) config.targets[i].label = a.label.trim() || keyOf(config.targets[i]);
      changed();
      return;
    }
    case 'set_person_sound': {
      const i = find(a.key);
      if (i >= 0) {
        if (a.spec) config.targets[i].sound = a.spec;
        else delete config.targets[i].sound;
      }
      changed();
      return;
    }
    case 'update_settings':
      deepMerge(config, a.patch);
      changed();
      return;
    case 'toggle_quiet':
      config.general.quiet = !config.general.quiet;
      changed();
      return config.general.quiet;
    case 'complete_setup':
      config.general.setup_complete = true;
      changed();
      return;
    case 'start_watching': {
      if (!config.targets.length) throw 'Add at least one person to watch first.';
      snap.watching = true;
      snap.status = 'connecting';
      changed();
      setTimeout(() => {
        snap.status = 'watching_channel';
        snap.detail = 'General';
        snap.members = [member(config.targets[1]), member(config.targets[0])];
        changed();
      }, 800);
      setTimeout(() => {
        if (!snap.watching) return;
        const t = config.targets[3];
        snap.members = [...snap.members, member(t)];
        snap.activity = [{ at: '21:04', key: keyOf(t), label: t.label, what: 'joined' }, ...snap.activity];
        changed();
        emit('alert', { key: keyOf(t), label: t.label });
      }, 2600);
      return;
    }
    case 'stop_watching':
      snap.stopping = true;
      changed();
      setTimeout(() => {
        Object.assign(snap, { watching: false, stopping: false, status: 'stopped', detail: '', members: [] });
        changed();
      }, 300);
      return;
    case 'test_sound':
      return 0.72;
    case 'save_credentials':
      config.client_id = a.clientId;
      if (a.clientSecret) snap.has_secret = true;
      changed();
      return;
    case 'reveal_secret':
      return 'example-secret-value';
    case 'test_connection':
      await new Promise((r) => setTimeout(r, 700));
      return 'you';
    case 'import_preview':
      return {
        kind: 'v1_config', kind_label: 'VC Alert 1 config', name: null, total: 5, new: 3, same: 1, different: 1,
        rows: [
          { key: 'lumen_88', label: 'lumen_88', username: 'lumen_88', id: null, status: 'new', mine_label: null, has_sound: false, sound_differs: false },
          { key: 'pineapple', label: 'Robin', username: 'pineapple', id: null, status: 'new', mine_label: null, has_sound: true, sound_differs: false },
          { key: 'cobalt.j', label: 'cobalt.j', username: 'cobalt.j', id: null, status: 'new', mine_label: null, has_sound: false, sound_differs: false },
          { key: 'maria_k', label: 'Mari', username: 'maria_k', id: null, status: 'different', mine_label: 'Maria', has_sound: false, sound_differs: true },
          { key: 'moderator', label: 'Moderator', username: 'moderator', id: null, status: 'same', mine_label: 'Moderator', has_sound: false, sound_differs: false },
        ],
        skipped: [{ value: 'bad name', reason: 'not a valid username or user ID' }],
        has_settings: true, has_credentials: true, has_token: a.migration, has_avatars: a.migration,
        missing_sounds: ['The Twins'], embedded_sounds: 0,
      } satisfies Preview;
    case 'import_apply':
      return { added: a.options.selected.length, updated: a.options.theirs.length, removed: 0, pictures: 0, backup: null };
    case 'export_text':
      return JSON.stringify({ vc_alert: { kind: a.options.kind, version: 2 }, targets: config.targets }, null, 2);
    case 'export_filename':
      return 'vc-alert-list-2026-10-21.json';
    case 'list_backups':
      return [{ name: 'settings-2026-10-21_14-03-00.json', when: '2026-10-21 14:03:00' }];
    case 'clear_pictures':
      return 3;
    case 'overlay_refresh': {
      const shown = (qp.get('n') ? people.slice(0, Number(qp.get('n'))) : people.slice(0, 5)).map((t) => ({ t, key: keyOf(t) }));
      emit('overlay', overlayModel(shown, null));
      if (qp.get('join') !== '0') {
        setTimeout(() => emit('overlay', overlayModel([...shown, { t: people[6], key: keyOf(people[6]) }], keyOf(people[6]))), 1400);
      }
      return;
    }
    case 'editor_refresh':
      emit('editor', {
        px: config.overlay.avatar_px, layout: config.overlay.layout, slot: config.overlay.avatar_px + 4, width: 0, height: 0,
        cells: people.slice(0, 3).map((t, i) => ({ x: 2 + i * (config.overlay.avatar_px + 8), y: 2, item: { kind: 'person', key: keyOf(t), label: t.label, username: t.username ?? '' } })),
        readout: 'Top left corner · 24 px from the left, 24 px from the top',
      });
      return;
    case 'open_position_editor':
      if (snap.watching) throw 'Stop watching first. The overlay can only be moved in setup mode.';
      return;
    default:
      return;
  }
}

// ------------------------------------------------ overlay preview model --
function overlayModel(members: { t: Target; key: string }[], flash: string | null) {
  const o = config.overlay;
  const style = o.style;
  const RING = 2, PAD = 2;
  const max = o.max_shown;
  const items: any[] = members.slice(0, max).map(({ t, key }) => ({ kind: 'person', key, label: t.label, username: t.username ?? '' }));
  if (members.length > max) items.push({ kind: 'badge', count: members.length - max, names: members.slice(max).map((m) => m.t.username ?? m.key) });
  const px = style === 'dots' ? Math.max(6, Math.round(o.avatar_px * 0.4)) : o.avatar_px;
  const n = Math.max(1, items.length);
  const cols = o.layout === 'column' ? 1 : o.layout === 'grid' ? (n <= 2 ? n : Math.min(4, Math.ceil(n / 2))) : n;
  const rows = Math.ceil(n / cols);
  const slot = px + 2 * RING;
  const gap = Math.max(3, Math.floor(px / 6));
  let w = 2 * PAD + cols * slot + (cols - 1) * gap;
  let h = 2 * PAD + rows * slot + (rows - 1) * gap;
  if (style === 'count') {
    const hh = Math.max(18, o.avatar_px + 4);
    w = 2 * PAD + Math.round(hh * 1.9);
    h = 2 * PAD + hh;
  }
  const side = o.layout === 'column' ? 'right' : 'below';
  const canvas = { width: Math.max(w, 240) + (side === 'right' ? 240 : 0), height: h + (side === 'below' ? 58 : 0), bar_x: 24, bar_y: 24 };
  canvas.width += 24; canvas.height += 24;
  const cells = style === 'count' ? [] : items.map((item, i) => ({
    x: canvas.bar_x + PAD + (i % cols) * (slot + gap),
    y: canvas.bar_y + PAD + Math.floor(i / cols) * (slot + gap),
    item,
    bubble: item.kind === 'badge'
      ? [`+${item.count} more`, item.names.slice(0, 4).map((x: string) => '@' + x).join(', ')]
      : item.label.toLowerCase() !== (item.username || item.key).toLowerCase() ? [item.label, '@' + (item.username || item.key)] : ['@' + (item.username || item.key), ''],
    avatar: null,
  }));
  return {
    style, px, slot, opacity: o.opacity, corner: o.corner, side, canvas, bar_w: w, bar_h: h, cells,
    count: members.length,
    count_bubble: [`${members.length} in your channel`, members.slice(0, 4).map((m) => '@' + (m.t.username ?? m.key)).join(', ')],
    flash,
  };
}

/** Scenes to judge overlay readability in the browser preview (?bg=bright|dark|busy). */
export function previewBackdrop(kind: string | null): string {
  switch (kind) {
    case 'bright':
      return 'linear-gradient(160deg, #e9f4ff, #fff6dd 60%, #ffffff)';
    case 'busy':
      return 'repeating-linear-gradient(45deg, #ff5f6d 0 18px, #ffc371 18px 36px, #2bc0e4 36px 54px, #1a1a1a 54px 72px)';
    case 'dark':
      return 'radial-gradient(circle at 70% 70%, #1d2a3a, #05070a 70%)';
    default:
      return '';
  }
}
