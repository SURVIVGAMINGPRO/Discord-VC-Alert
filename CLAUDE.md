# CLAUDE.md: VC Alert 2

VC Alert is a small desktop app that gives a quiet beep, and shows a tiny
always-on-top bar of profile pictures, when chosen people join the Discord
voice channel the user is in. Version 2 is a Tauri 2 app: a Rust engine plus a
Svelte 5 UI. It replaces the Python/Tk version 1, which lives in `vc_alert_OLD/`
(git-ignored, it holds a real client secret and login token). The full v1
behaviour spec is `vc_alert_OLD/CLAUDE_v1_spec.md`; where this file is silent,
v1's spec still describes the intended behaviour.

## 1. Hard rules (never break these)

1. **No bot, no self-bot.** Never read, use or ask for the user's Discord
   account token, never connect to the Discord gateway as the user.
2. **Only Discord's documented local RPC** to the user's own desktop app, plus
   the OAuth2 token exchange with `discord.com` and plain GETs of public
   pictures from `cdn.discordapp.com`. No other network traffic: no telemetry,
   no auto-updater, no web fonts, no CDNs.
3. **Read-only RPC.** The only commands ever sent are `AUTHORIZE`,
   `AUTHENTICATE`, `SUBSCRIBE`, `UNSUBSCRIBE`, `GET_SELECTED_VOICE_CHANNEL`
   (plus handshake and `PONG`). `crates/vc-core/tests/watcher.rs` enforces it.
4. **Discreet.** No "birthday" or "surprise" wording anywhere: UI, examples,
   defaults, file names. The example name is `Moderator`. No emoji.
5. **Match on username or numeric ID only**, never display names or nicknames.
6. **The live overlay can't be dragged or clicked.** It is click-through;
   only the position editor moves it.
7. **Exports never contain the Discord login token.** A shared watch list never
   contains credentials, file paths or settings.
8. **Be honest** about what has and hasn't been run on real Discord, macOS and
   Windows (section 12).

## 1b. Versioning

Every fix bumps the patch number: 2.0.1 was the Start/Stop freeze fix, 2.0.2
the "first launch looks like a crash" fix, 2.0.3 the one-row header and real
profile pictures everywhere, 2.0.4 the list as cards, 2.0.5 the scroll progress bar instead of a
scrollbar, 2.0.6 no console window on Windows, 2.0.7 centred lamp, even avatar spacing
and no Space switching on macOS, 2.0.8 snooze, leave sound, Quiet reminder
options and 30 px pictures, 2.0.9 stacked toasts with a countdown bar, 2.1.0
the logo, its launch zoom, the card hover shadow and GitHub Releases; fixes
from here are 2.1.x. Change it in
`Cargo.toml` (`[workspace.package]`), `package.json` and
`src-tauri/tauri.conf.json` together (plus the version in `src/lib/mock.ts`),
and add an entry at the top of `CHANGELOG.md` in plain, user-facing words.
Then publish a GitHub Release (section 11) so the README's download buttons
serve it.

## 2. Layout

```
crates/vc-core/      Rust engine, no UI (unit + integration tests)
  src/config.rs      schema, defaults, v1 validation rules (clamps, parse_identifier)
  src/store.rs       settings.json store, atomic saves
  src/transfer.rs    import (detect/preview/apply) and export, backups
  src/discord.rs     IPC transport, RPC session, OAuth, watcher state machine
  src/sound.rs       beep generator (exact v1 formula), sound bank, serial player
  src/avatars.rs     CDN picture cache (index.json, 60 s back-off)
  src/geometry.rs    overlay geometry (v1 port) + bubble canvas
  tests/             watcher vs a mock Discord; real v1 config import
src-tauri/           Tauri shell
  src/lib.rs         setup, windows, plugins, run loop
  src/state.rs       AppState, Snapshot (what the UI renders), watcher Bridge
  src/commands.rs    every command the UI calls
  src/overlay.rs     overlay window: model, placement, cursor-polled hover
  src/editor.rs      position editor (ghost + panel windows)
  src/tray.rs        menu-bar / notification-area icon
  src/secrets.rs     keychain storage (one item: client secret + token)
  src/platform.rs    NSWindow / Win32 tweaks for the overlay
src/                 Svelte 5 + TypeScript UI (one bundle, routed by window label)
  styles/tokens.css  THE design tokens (colour, type, space, radius, motion)
  app.css            base styles only (no colours of its own)
  lib/               api (invoke + mock fallback), app state, motion, util, types, icons
  components/        base components
  views/             Main, PersonRow, Settings, Overlay, EditorGhost, EditorPanel
  sheets/            AddPerson, PersonSound, ImportSheet, ExportSheet, PasteList, Wizard
design/preview.html  design-direction preview (served by `npm run dev`)
scripts/make-icons.mjs  renders app + tray icons from assets/appicon.svg
.github/workflows/build.yml  builds the macOS and Windows apps on GitHub
```

## 3. Commands

Rust was installed with rustup without touching the shell profile, so prefix
cargo commands with `export PATH="$HOME/.cargo/bin:$PATH"` (or add it to
`~/.zshrc`).

```
npm install                       # once
npm run dev                       # UI only, in a browser, with the mock backend
npx tauri dev                     # the real app (Rust + UI, hot reload)
npm run check                     # svelte-check: must be 0 errors, 0 warnings
npm run build                     # UI bundle
cargo test -p vc-core             # engine tests (47 at the time of writing)
cargo test -p vc-core --test avatars_live -- --ignored   # real CDN download (network)
CI=true npx tauri build --target universal-apple-darwin   # macOS .app + .dmg
npm run icons && npx tauri icon src-tauri/icons/app-icon.png -o src-tauri/icons
```

`CI=true` stops the DMG step from scripting Finder. The Windows build is in
section 11.

Dev tip: `VC_ALERT_SECRETS_FILE=1` keeps secrets in a private file instead of
the macOS Keychain, so rebuilt (re-signed) binaries don't trigger keychain
prompts. `VC_ALERT_DATA_DIR=/path` and `VC_ALERT_IPC_PATH=/path/to/socket`
override the data folder and the Discord socket.

## 4. Engine behaviour (vc-core)

Ported from v1 and covered by tests; keep them green.
- `parse_identifier`: 15+ ASCII digits is an ID; else a 2–32 character
  `[a-z0-9_.]` username (lowercased, leading `@` stripped).
- Validation and clamps match v1 (Python round-half-even for integers). The
  default beep is v2's **Classic: 1700 Hz, 90 ms, 3 beeps, 75%** (v1 used
  1000 Hz/180 ms/60%); missing or invalid beep values fall back to it.
- `beep_profiles`: up to 3 saved beeps (`{name, beep: {frequency_hz,
  duration_ms, repeats, volume}}`), names cleaned and capped at 12 characters,
  unique ignoring case. The rhythm (`gap_ratio`) stays universal.
- The watcher: handshake → token (saved → refresh → AUTHORIZE popup + code
  exchange) → subscribe to channel changes → follow the selected channel →
  presence in join order, alert with per-person cooldown, Quiet suppresses
  sound only. `Host::left(key)` fires when a watched person leaves your channel
  (never for your own channel switches, never in Quiet); the shell plays their
  leave sound (`sound::leave_beep`: one lower, quieter beep) if `leave_sound`. Reconnect every 5 s. Stop is a cancellation token and takes
  effect immediately on every platform (v1 couldn't on Windows).
- **v1 bug fixed:** v1 stayed on "connecting" if you weren't in a voice
  channel when pressing Start; v2 reports `idle_no_channel`.
- Sounds: beep is generated in memory (mono 16-bit 44.1 kHz, 5 ms fades);
  files may be WAV/MP3/OGG/FLAC (≤10 MB), copied into `sounds/` when chosen.
  One audio thread plays clips strictly in order; the device closes after 8 s idle.

## 5. Storage

- Data folder: `~/Library/Application Support/VC Alert/` (macOS),
  `%APPDATA%\VC Alert\` (Windows). A file named `portable` next to the .exe keeps
  everything beside the program instead.
- `settings.json` is v1's `config.json` shape **minus secrets**, plus
  `"vc_alert": {"version": 2}`, `overlay.style|display|hide_from_capture`,
  `general.close_to_tray|launch_at_login|hide_dock_icon|setup_complete`,
  `appearance.theme|density`, `general.quiet_reminder`, per-person
  `leave_sound` (only written when true) and `beep_profiles`. v1 ignores the
  extra keys. A broken file is
  moved aside (`settings.broken-<time>.json`), never overwritten.
- Secrets: one keychain item (service "VC Alert", account "discord") holding
  `{client_secret, token}`, so macOS asks at most once. Falls back to
  `secrets.json` (0600) if the keychain can't be used, with a notice in the UI.
- `backups/`: a copy of settings.json before every import or restore (last 10).
- `general.start_minimized` ("Start in the background") applies only when the
  login item opens VC Alert: it passes `--autostart`, and the app re-registers
  the login item at startup so older ones get the flag. A manual launch always
  shows and focuses the window. (Hidden + no Dock icon on a manual launch
  looked like a crash on first open.)
- `avatars/`: 128 px PNGs + `index.json` (v1's format). Pictures can only be
  fetched for watched people who have been in a voice channel with the user:
  the user ID and picture hash come from Discord's voice states. They show in
  the list, the sound sheet, the overlay and the position editor.

## 6. Import and export (transfer.rs)

- Detects: v1 config.json, v1 `target_user_ids`, v2 watch list
  (`vc_alert.kind = "watchlist"`), v2 backup (or a copy of settings.json), a
  plain JSON array. Paste of plain usernames goes through `add_many` instead.
- Untrusted input: ≤5 MB, ≤2000 people, labels cleaned (no control chars,
  ≤64 chars), unknown keys ignored, sound *paths* from shared files ignored;
  only embedded sounds (base64, ≤1 MB, must decode) are accepted.
- VC Alert 1 was never released publicly, so there is no "bring over v1" flow
  and no v1 export in the UI. The engine still reads v1-shaped files through
  the normal Import, and still has `migration`/`v1` code paths (unused by the UI).
- Preview → user picks Merge (default) or Replace, rows, and Mine/Theirs for
  conflicts; settings and credentials only if ticked (both off by default).
  Imported beep profiles merge with the user's (theirs fill free slots, names
  already taken are skipped). A backup is written before applying.
- Export kinds in the UI: `watchlist` (for friends) and `backup` (credentials
  only if ticked; file name ends `-private`). The `targets` array keeps v1's
  shape.

## 7. Windows and the UI contract

Windows (labels): `main`, `settings`, `overlay`, `editor-ghost`,
`editor-panel`. All load `index.html`; `App.svelte` routes by label (in a
browser, by `?view=`). Rust is the single source of truth: commands change
state, then a full `state` event (the Snapshot) goes to every window. Other
events: `alert`, `avatar`, `toast`, `overlay`, `overlay-hover`, `editor`,
`settings-tab`, `import-file`.

**Threads (the Start/Stop freeze):** from any thread but the main one,
Tauri's window/monitor/menu calls post to the main thread and wait. So:
never call into Tauri while holding an app lock (`store`, `ui`, `run`,
`displays`), and keep the main thread off our locks. Every command is `async`
(runs on the async runtime), tray menu handlers spawn their work, and windows
are created from async contexts (creating them from sync commands or event
handlers deadlocks on Windows). The display list is cached and refreshed in
`get_state` only.

**Snooze:** `snooze(key, mode)` with "hour" | "call" | "off". Snoozes live in
`Run.snoozes` (memory only) and silence that person's join and leave sounds;
"call" ends when you leave that channel (by name) or stop watching. The
Snapshot lists them as `snoozed: [{key, until}]` (`until` null = this call).

**Spaces (macOS):** the main and Settings windows use
`platform::follow_active_space` (MoveToActiveSpace), so opening VC Alert from
the Dock or menu bar brings the window to the current Space instead of
switching to the one it was last on.

**Do not touch `crates/vc-core` or `src-tauri` for UI work.** If a UI change
needs a new command, ask first. Tauri converts command args to camelCase
(`clientId`), but struct fields inside an object stay snake_case
(`import_settings`).

## 8. Design system ("Lookout")

A quiet night-shift panel built from the icon's blurple. Mode: Operate (tool UI).

- **Every value comes from `src/styles/tokens.css`.** No raw hex, no
  hand-typed durations or easings in components. `motion.ts` reads tokens at
  runtime to build the JS easing functions Svelte transitions need.
- Themes: `ink` (default dark), `graphite`, `paper` (light). Saved values map:
  `onyx`→Ink, `graphite`→Graphite, `paper`→Paper, `night`→Ink,
  `system`→Ink/Paper from the OS (`resolveTheme` in `lib/app.svelte.ts`).
- Colour meaning: blurple `--accent` is the only action colour, the focus
  ring and the list's scroll progress bar; green `--live` only ever means "someone is live in your call"; red only
  for removing things.
- Type: the platform font for the UI; **Atkinson Hyperlegible Mono only for
  usernames and IDs** (class `.id`). Sizes 11/12/13/15/18.
- Radius by importance: 4 tags, 6 buttons/inputs, 10 rows, 14 sheets, full for
  avatars, lamps, pills (the status pill, the "Custom" tag) and round icon
  buttons. 4 px spacing grid.
- Main window header (layout from the early mockup): one row with the status
  pill (lamp centred in its round end, state, channel, "n here"), round Quiet
  and Settings buttons, then
  Start/Stop. Longer text (what went wrong, what to do, Quiet) unfolds on a
  line below. Only the channel name truncates (full name in the tooltip).
- The list (layout from the same mockup): headings "In your channel · 2";
  each person is a card (surface, hairline edge, 8 px apart, no dividers):
  `--avatar` picture (30 px; 34 comfortable) centred in a `--row-h` card
  with the same space to its left as above and below, name, then
  @username/ID and, while in the channel, "joined
  2 min ago" in green. Letters (no picture yet) have the tooltip "Their
  picture appears after their first call with you". Right side: "Snoozed ·
  52 min", else "Custom" or "Custom · Urgent" (preset or profile name); on hover
  or focus a round More (⋯) button takes its place (the tag hides at once).
  Sound… lives in that menu. The list has no scrollbar (it made the right-hand
  gap uneven): a 2 px blurple bar on the footer's top line fills left to right
  with the scroll position, and only shows when the list can scroll.
- Copy: plain, sentence case, no "please", controls name their action and the
  resulting toast reuses the verb.
- The overlay uses only `--ov-*` tokens (theme-independent): dark plates with a
  light edge so it reads on black, white and busy frames.

### Motion rules (Animate UI values, written as CSS + Svelte)
- Motion level "subtle"; the one bigger moment is "someone joined".
- Only transform, opacity and filter animate (height only for unfolding
  sections). Never top/left/margin/width; button resizes use FLIP on a
  background layer.
- Reduced motion: fades allowed; nothing moves, scales or shakes; halos become
  static rings. CSS uses `@media (prefers-reduced-motion: reduce)`, JS uses
  `reduced()`.
- Focus rings appear instantly and are never clipped (list has scroll-padding).
- Anything fading out gets `inert`; every entrance clears it (reversed exits
  must be clickable again).
- Interrupting retargets (FLIP measures the current, in-flight box).
- List: one keyed `{#each}` for both sections so rows `animate:glide` between
  them; rows are opaque and the furthest-moving row is drawn on top; filtering
  while typing doesn't animate.
- Overlay: only the join halo, the count roll + bump, and the bubble grow and
  glide. Nothing on the overlay loops.
- Launch: the logo (`components/Logo.svelte`, `assets/appicon.svg`) shows for
  `--t-launch-hold`, then Animista's slide-out-fwd-center (translateZ to
  `--launch-depth` under `--launch-perspective`, fading, `--t-launch` on
  `--ease-launch`, the user's exit curve) while the backdrop fades; a timer
  removes it. Reduced motion: it only fades. The logo also sits in About and
  the empty list.
- Card hover: Animista's shadow-inset-center (`inset 0 0 14px --card-inset`,
  `--t-inset` on `--ease-inset`); the keyboard focus ring skips that fade.
- Toasts (`components/Toasts.svelte` + `Toast.svelte`, Animate UI's toast as
  CSS): a stack, newest in front, up to 3 (`TOAST_LIMIT`); behind ones scale
  `max(0.9, 1 - i * 0.05)` and peek `--toast-peek` above; hover or focus opens
  it (`--toast-gap`, `--stagger-stack` front to back, only while opening or
  closing) and pauses every timer. In: rise their own height from
  `--toast-from` out of `--blur-surface` on `--spring-toast`; out: in place on
  `--t-dismiss`/`--ease-away`. A 2 px blurple bar counts down (a Web
  Animation paused with the timer). `toast(kind, message, {detail?, action?,
  ms?})` splits "Title. More words." into a title and a muted second line.

## 9. Verifying UI changes

`npm run dev`, then open `http://localhost:1420/?view=main` (or
`view=settings#overlay`, `view=overlay`). The mock backend (`lib/mock.ts`)
takes URL params:
`status=connecting|idle|watching|watching0|reconnecting|reconnecting2|setup_error`,
`quiet=1`, `wizard=1`, `nocreds=1`, `notice=1`, `empty=1`, `mask=1`,
`theme=onyx|graphite|paper|system`, `density=comfortable`, overlay
`style=pictures|count|dots`, `px=8..48`, `n=…`, `join=0`,
`bg=bright|dark|busy`, `overlay=off`, `pics=1` (stand-in pictures for
everyone but the last person), `snoozed=1` (Sam for 52 min, pixel_fox for
this call), `reminder=off` (Quiet reminder switched off). Check 380 × 520 (the minimum window
size), both Ink and Paper, Tab order, and mid-animation frames.
Then run the detector:
`~/.claude/plugins/cache/impeccable/impeccable/4.7.0/skills/impeccable/scripts/impeccable detect --json src design/preview.html`
(two narrow ignores are recorded in `.impeccable/config.json`: the Avatar
`<img>` false alarm and the preview's striped test background).

## 10. Overlay mechanics

Transparent, click-through (`set_ignore_cursor_events`), always on top, never
focusable, on all Spaces; macOS: status window level + full-screen auxiliary;
Windows: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` and topmost re-asserted every
2 s. Hover: Rust polls the cursor every 60 ms and sends the hovered index.
The window is bigger than the bar (room for the bubble), anchored so the bar
keeps v1's corner geometry. "Hide from screen sharing" applies content
protection to every VC Alert window.

## 11. Packaging

- **macOS:** `CI=true npx tauri build --target universal-apple-darwin` →
  `target/universal-apple-darwin/release/bundle/{macos/VC Alert.app,dmg/*.dmg}`.
  Ad-hoc signed only: on another Mac, open it via System Settings → Privacy &
  Security → Open Anyway (or sign + notarize with an Apple Developer ID).
- **Windows:** push to GitHub and run `.github/workflows/build.yml` (portable
  `vc-alert.exe`, NSIS installer, MSI). Cross-building on the Mac also works
  with `cargo-xwin` + `nsis` + LLVM, but it downloads Microsoft's CRT/SDK, which
  means accepting Microsoft's license: ask the user first. The .exe needs
  WebView2 (built into Windows 11; the installer fetches it on Windows 10).
  Unsigned, so SmartScreen shows "More info → Run anyway".
- **Releases:** `gh release create vX.Y.Z` with the notes from CHANGELOG.md and
  these exact asset names, which the README's buttons link to through
  `releases/latest/download/…`: `VC-Alert-macOS.dmg` (the universal DMG),
  `VC-Alert-Windows-setup.exe` (NSIS), `VC-Alert-Windows.msi`,
  `VC-Alert-Windows-portable.exe` (`vc-alert.exe`). Renaming any of them
  breaks the download buttons.

## 12. Verified vs not verified

Verified here: engine tests (config, store, transfer, geometry, tone, avatars,
watcher vs mock Discord incl. read-only commands and instant Stop), the user's
real v1 config imports cleanly, `npm run check`/`build`, UI behaviour in a
browser with the mock backend, and the universal macOS bundle builds and
launches (main window, hidden overlay at the status window level, tray icon).
The GitHub workflow builds the Windows installer, MSI and portable .exe, and
`npm run check` + the engine tests pass on its Windows machine. Release .exe
files are Windows GUI programs (`windows_subsystem = "windows"` in
`src-tauri/src/main.rs`); without it a console window opens beside the app.

Not verified yet: a real Discord session in v2 (v1 did authenticate on this
Mac once), the Windows build running on Windows (named pipe, WebView2, tray,
overlay flags), overlay behaviour over full-screen apps on macOS, content
protection against modern capture tools, keychain prompts on unsigned
updates, multi-monitor placement.
