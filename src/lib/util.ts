import type { Beep, Config, Target } from './types';

// v1 letter-circle colour index (the colours are the --id-0..5 tokens).
export function avatarIndex(key: string): number {
  if (/^\d+$/.test(key)) return Number(BigInt(key) % 6n);
  let sum = 0;
  [...key].forEach((c, i) => (sum += c.codePointAt(0)! * (i + 1)));
  return sum % 6;
}

export const avatarColor = (key: string) => `var(--id-${avatarIndex(key)})`;

export function initialOf(label: string): string {
  const t = label.trim();
  return t ? [...t][0].toUpperCase() : '?';
}

export const keyOf = (t: Target) => t.id ?? t.username ?? '';

// v1 masking: keep the first 2 and last 2 characters.
export function mask(name: string): string {
  const c = [...name];
  if (c.length <= 4) return (c[0] ?? '') + '•'.repeat(Math.max(0, c.length - 1));
  return c.slice(0, 2).join('') + '•'.repeat(c.length - 4) + c.slice(-2).join('');
}

// Same rules as vc-core parse_identifier, for instant feedback in the UI.
export function parseIdent(text: string): { id?: string; username?: string } | null {
  const raw = text.trim().replace(/^@+/, '').trim();
  if (raw.length >= 15 && /^\d+$/.test(raw)) return { id: raw };
  const name = raw.toLowerCase();
  if (name.length >= 2 && name.length <= 32 && /^[a-z0-9_.]+$/.test(name)) return { username: name };
  return null;
}

export const PARSE_ERROR =
  'Enter a Discord username (letters, numbers, _ and . only; this is the unique username, not the display name) or a numeric user ID.';

export function describeBeep(b: Beep): string {
  return `${b.frequency_hz} Hz · ${b.duration_ms} ms · ${b.repeats} ${b.repeats === 1 ? 'beep' : 'beeps'}`;
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function describeUniversal(cfg: Config): string {
  return cfg.sound_file ? baseName(cfg.sound_file) : describeBeep(cfg.beep);
}

/** The second line under a name: @username (unless it equals the label), or ID. */
export function subline(t: Target, masked: boolean): string | null {
  if (t.id) {
    const id = masked ? mask(t.id) : t.id;
    return `ID ${id.slice(0, 4)}…${id.slice(-4)}`;
  }
  const u = t.username ?? '';
  if (!masked && u.toLowerCase() === t.label.trim().toLowerCase()) return null;
  return '@' + (masked ? mask(u) : u);
}

export function shownLabel(t: Target, masked: boolean): string {
  const k = keyOf(t);
  if (masked && t.label.trim().toLowerCase() === k.toLowerCase()) return mask(t.label);
  return t.label;
}

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

/** A→Z by label (case-insensitive), ties by key. Display-only. */
export function sortTargets(list: Target[]): Target[] {
  return [...list].sort((a, b) => collator.compare(a.label, b.label) || keyOf(a).localeCompare(keyOf(b)));
}

export function matchesQuery(t: Target, q: string): boolean {
  const s = q.trim().replace(/^@/, '').toLowerCase();
  if (!s) return true;
  return [t.label, t.username ?? '', t.id ?? ''].some((v) => v.toLowerCase().includes(s));
}

export function cornerText(corner: string, offset: [number, number]): string {
  const [v, h] = corner.split('-');
  const s = `${v} ${h} · ${offset[0]} px, ${offset[1]} px from the corner`;
  return s[0].toUpperCase() + s.slice(1);
}

export function ago(ms: number): string {
  const m = Math.floor(ms / 60000);
  if (m < 1) return 'joined just now';
  if (m < 60) return `joined ${m} min ago`;
  const h = Math.floor(m / 60);
  return `joined ${h} h ago`;
}

export const BEEP_PRESETS: { name: string; beep: Omit<Beep, 'gap_ratio'> }[] = [
  { name: 'Classic', beep: { frequency_hz: 1700, duration_ms: 90, repeats: 3, volume: 0.75 } },
  { name: 'Ping', beep: { frequency_hz: 1700, duration_ms: 70, repeats: 1, volume: 0.8 } },
  { name: 'Double', beep: { frequency_hz: 1200, duration_ms: 90, repeats: 2, volume: 0.7 } },
  { name: 'Chime', beep: { frequency_hz: 1350, duration_ms: 160, repeats: 2, volume: 0.6 } },
  { name: 'Soft', beep: { frequency_hz: 650, duration_ms: 220, repeats: 2, volume: 0.45 } },
  { name: 'Urgent', beep: { frequency_hz: 2000, duration_ms: 60, repeats: 5, volume: 0.9 } },
];

/** Same pitch, length, repeats and volume (the rhythm is universal). */
export function sameBeep(a: Omit<Beep, 'gap_ratio'>, b: Omit<Beep, 'gap_ratio'>): boolean {
  return a.frequency_hz === b.frequency_hz && a.duration_ms === b.duration_ms && a.repeats === b.repeats && a.volume === b.volume;
}

/** Saved profiles: the same limits as vc-core's config.rs. */
export const MAX_PROFILES = 3;
export const PROFILE_NAME_MAX = 12;

/** v1 Test-button lock: max(duration, 0.4) s + 150 ms. */
export function lockMs(duration: number): number {
  return Math.max(duration, 0.4) * 1000 + 150;
}
