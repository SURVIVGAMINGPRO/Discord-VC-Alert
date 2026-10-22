// Shapes shared with the Rust side (see src-tauri/src/state.rs and vc-core).

export type Status =
  | 'stopped'
  | 'connecting'
  | 'idle_no_channel'
  | 'watching_channel'
  | 'reconnecting'
  | 'setup_error';

export interface Beep {
  frequency_hz: number;
  duration_ms: number;
  repeats: number;
  gap_ratio: number;
  volume: number;
}

export type PartialBeep = Partial<Beep>;

/** A saved beep (the rhythm, gap_ratio, stays universal). */
export interface BeepProfile {
  name: string;
  beep: { frequency_hz: number; duration_ms: number; repeats: number; volume: number };
}

export interface SoundSpec {
  file?: string;
  beep?: PartialBeep;
}

export interface Target {
  label: string;
  id?: string;
  username?: string;
  sound?: SoundSpec;
}

export interface OverlaySettings {
  enabled: boolean;
  corner: string;
  offset: [number, number];
  avatar_px: number;
  layout: 'row' | 'column' | 'grid';
  max_shown: number;
  opacity: number;
  bubble: 'username' | 'label' | 'both';
  hide_when_empty: boolean;
  style: 'pictures' | 'count' | 'dots';
  display: string;
  hide_from_capture: boolean;
}

export interface General {
  start_watching_on_launch: boolean;
  start_minimized: boolean;
  mask_usernames: boolean;
  quiet: boolean;
  close_to_tray: boolean;
  launch_at_login: boolean;
  hide_dock_icon: boolean;
  setup_complete: boolean;
}

export interface Appearance {
  theme: 'onyx' | 'graphite' | 'night' | 'paper' | 'system';
  density: 'compact' | 'comfortable';
}

export interface Config {
  client_id: string;
  redirect_uri: string;
  scopes: string[];
  targets: Target[];
  alert_if_already_present: boolean;
  cooldown_seconds: number;
  sound_file: string;
  beep: Beep;
  beep_profiles: BeepProfile[];
  overlay: OverlaySettings;
  general: General;
  appearance: Appearance;
}

export interface Member {
  key: string;
  id: string;
  username: string;
  avatar: string | null;
}

export interface Activity {
  at: string;
  key: string;
  label: string;
  what: 'joined' | 'left';
}

export interface DisplayInfo {
  name: string;
  label: string;
  primary: boolean;
}

export interface Snapshot {
  config: Config;
  has_secret: boolean;
  watching: boolean;
  stopping: boolean;
  status: Status;
  detail: string;
  members: Member[];
  activity: Activity[];
  platform: 'macos' | 'windows' | 'linux';
  version: string;
  data_dir: string;
  displays: DisplayInfo[];
  notice: string | null;
  editor_open: boolean;
}

export interface PreviewRow {
  key: string;
  label: string;
  username: string | null;
  id: string | null;
  status: 'new' | 'same' | 'different';
  mine_label: string | null;
  has_sound: boolean;
  sound_differs: boolean;
}

export interface Preview {
  kind: string;
  kind_label: string;
  name: string | null;
  total: number;
  new: number;
  same: number;
  different: number;
  rows: PreviewRow[];
  skipped: { value: string; reason: string }[];
  has_settings: boolean;
  has_credentials: boolean;
  has_token: boolean;
  has_avatars: boolean;
  missing_sounds: string[];
  embedded_sounds: number;
}

export interface ExportOptions {
  kind: 'watchlist' | 'backup';
  keys: string[] | null;
  embed_sounds: boolean;
  include_credentials: boolean;
  name: string | null;
}

export type Item =
  | { kind: 'person'; key: string; label: string; username: string }
  | { kind: 'badge'; count: number; names: string[] }
  | { kind: 'empty' };

export interface OverlayCell {
  x: number;
  y: number;
  item: Item;
  bubble: [string, string];
  avatar: string | null;
}

export interface OverlayModel {
  style: 'pictures' | 'count' | 'dots';
  px: number;
  slot: number;
  opacity: number;
  corner: string;
  side: 'above' | 'below' | 'left' | 'right';
  canvas: { width: number; height: number; bar_x: number; bar_y: number };
  bar_w: number;
  bar_h: number;
  cells: OverlayCell[];
  count: number;
  count_bubble: [string, string];
  flash: string | null;
}

export interface EditorModel {
  px: number;
  layout: 'row' | 'column' | 'grid';
  slot: number;
  width: number;
  height: number;
  cells: { x: number; y: number; item: Item }[];
  readout: string;
}
