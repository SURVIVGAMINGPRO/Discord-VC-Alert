<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../components/Button.svelte';
  import CredentialsForm from '../components/CredentialsForm.svelte';
  import Icon from '../components/Icon.svelte';
  import Segmented from '../components/Segmented.svelte';
  import SettingRow from '../components/SettingRow.svelte';
  import Slider from '../components/Slider.svelte';
  import SoundControls from '../components/SoundControls.svelte';
  import Switch from '../components/Switch.svelte';
  import Toasts from '../components/Toasts.svelte';
  import ImportSheet from '../sheets/ImportSheet.svelte';
  import ExportSheet from '../sheets/ExportSheet.svelte';
  import PasteList from '../sheets/PasteList.svelte';
  import { app, toast } from '../lib/app.svelte';
  import { call, closeWindow, errText, on, pickFile } from '../lib/api';
  import { fold, slideHighlight, swapIn, unfold } from '../lib/motion';
  import { cornerText } from '../lib/util';
  import type { Beep, Config } from '../lib/types';

  const snap = $derived(app.snap!);
  const cfg = $derived(snap.config);
  const mac = $derived(snap.platform === 'macos');

  const TABS = [
    { id: 'sound', label: 'Sound', icon: 'volume_up' },
    { id: 'overlay', label: 'Overlay', icon: 'radio_button_checked' },
    { id: 'general', label: 'General', icon: 'settings' },
    { id: 'connection', label: 'Connection', icon: 'link' },
    { id: 'data', label: 'Import and export', icon: 'upload' },
    { id: 'about', label: 'About', icon: 'info' },
  ] as const;
  type Tab = (typeof TABS)[number]['id'];

  const fromHash = () => {
    const h = location.hash.slice(1);
    return (TABS.some((t) => t.id === h) ? h : 'sound') as Tab;
  };
  let tab = $state<Tab>(fromHash());

  // --------------------------------------------- sidebar highlight
  let nav: HTMLElement | undefined = $state();
  let hl: HTMLSpanElement | undefined = $state();
  let mounted = false;
  $effect(() => {
    void tab;
    slideHighlight(hl, nav?.querySelector<HTMLElement>('[aria-current="page"]'), mounted);
    mounted = true;
  });

  function navKey(e: KeyboardEvent) {
    const i = TABS.findIndex((t) => t.id === tab);
    const d = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    tab = TABS[(i + d + TABS.length) % TABS.length].id;
    queueMicrotask(() => nav?.querySelector<HTMLElement>('[aria-current="page"]')?.focus());
  }

  // ------------------------------------------------ overlay preview
  let previewOn: boolean | null = null;
  $effect(() => {
    const on = tab === 'overlay' && !snap.watching;
    if (on !== previewOn) {
      previewOn = on;
      call('set_overlay_preview', { on });
    }
  });

  // --------------------------------------------------------- saving
  const patch = (p: Record<string, unknown>) => call('update_settings', { patch: p }).catch((e) => toast('error', errText(e)));
  const set = <K extends keyof Config>(section: K, values: Partial<Config[K]>) => patch({ [section]: values });

  let soundMode = $state<'beep' | 'file'>(app.snap!.config.sound_file ? 'file' : 'beep');
  function universal(m: 'universal' | 'beep' | 'file', b: Beep, f: string) {
    soundMode = m === 'file' ? 'file' : 'beep';
    patch({ beep: b, sound_file: m === 'file' ? f : '' });
  }

  // ------------------------------------------------------------ data
  let importing = $state<{ path?: string; text?: string } | null>(null);
  let pasting = $state(false);
  let exporting = $state(false);
  let backups = $state<{ name: string; when: string }[]>([]);
  const loadBackups = () => call<{ name: string; when: string }[]>('list_backups').then((b) => (backups = b));
  $effect(() => {
    if (tab === 'data') loadBackups();
  });

  async function importFile() {
    const path = await pickFile('Import a list', [{ name: 'JSON', extensions: ['json'] }]);
    if (path) importing = { path };
  }

  let resetDone = $state(false);
  async function resetLogin() {
    try {
      await call('reset_login');
      resetDone = true;
      setTimeout(() => (resetDone = false), 2500);
      toast('ok', 'Discord will ask you to approve VC Alert again the next time you start watching.');
    } catch (e) {
      toast('error', errText(e));
    }
  }

  const themeValue = $derived(cfg.appearance.theme === 'night' ? 'onyx' : cfg.appearance.theme);

  onMount(() => {
    const off = on<string>('settings-tab', (t) => {
      if (TABS.some((x) => x.id === t)) tab = t as Tab;
    });
    return () => {
      off.then((f) => f());
      call('set_overlay_preview', { on: false });
    };
  });

  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape' && !importing && !pasting && !exporting && !document.querySelector('[role="dialog"], [role="menu"]')) closeWindow();
  }
</script>

<svelte:window onkeydown={keys} onhashchange={() => (tab = fromHash())} />

<div class="settings">
  <!-- Arrow keys move between sections (roving tabindex). -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <nav bind:this={nav} aria-label="Settings sections" onkeydown={navKey}>
    <span class="hl" bind:this={hl} aria-hidden="true"></span>
    {#each TABS as t}
      <button type="button" aria-current={tab === t.id ? 'page' : undefined} tabindex={tab === t.id ? 0 : -1} onclick={() => (tab = t.id)}>
        <Icon name={t.icon} size={18} />
        {t.label}
      </button>
    {/each}
    <p class="saves">Changes save as you make them.</p>
  </nav>

  <main>
    {#key tab}
      <section in:swapIn aria-labelledby="h-{tab}">
        {#if tab === 'sound'}
          <h1 id="h-sound">Sound</h1>
          <h2>Universal sound</h2>
          <p class="lead">Plays for everyone, unless someone has their own sound from the speaker button in your list.</p>
          <SoundControls
            modes={[
              { value: 'beep', label: 'Beep' },
              { value: 'file', label: 'Sound file' },
            ]}
            mode={soundMode}
            beep={cfg.beep}
            file={cfg.sound_file}
            onchange={universal}
            ontest={() => call<number>('test_sound', { spec: null })}
          />
          <div class="rows">
            <SettingRow title="Beep if they're already there" sub="Also alert when you join a channel they're already in.">
              <Switch label="Beep if they're already there" checked={cfg.alert_if_already_present} onchange={(v) => patch({ alert_if_already_present: v })} />
            </SettingRow>
            <SettingRow title="Ignore repeats" sub="The shortest time between two alerts for the same person." stack>
              <Slider label="Ignore repeats" value={Math.min(30, cfg.cooldown_seconds)} min={0} max={30} step={1} format={(v) => (v === 0 ? 'Off' : `${v} s`)} onchange={(v) => patch({ cooldown_seconds: v })} />
            </SettingRow>
          </div>
        {:else if tab === 'overlay'}
          <h1 id="h-overlay">Overlay</h1>
          <p class="lead">A tiny bar on top of every app, showing who from your list is in your channel.{snap.watching ? '' : ' A preview shows while this page is open.'}</p>
          <div class="rows">
            <SettingRow title="Show the overlay">
              <Switch label="Show the overlay" checked={cfg.overlay.enabled} onchange={(v) => set('overlay', { enabled: v })} />
            </SettingRow>
            <SettingRow title="Position" sub={snap.watching ? 'Stop watching to move it.' : cornerText(cfg.overlay.corner, cfg.overlay.offset)}>
              <Button small disabled={snap.watching} onclick={() => call('open_position_editor').catch((e) => toast('info', errText(e)))}>Position…</Button>
            </SettingRow>
            <SettingRow title="Style">
              <Segmented
                label="Overlay style"
                value={cfg.overlay.style}
                options={[
                  { value: 'pictures', label: 'Pictures' },
                  { value: 'count', label: 'Count' },
                  { value: 'dots', label: 'Dots' },
                ]}
                onchange={(v) => set('overlay', { style: v })}
              />
            </SettingRow>
            <SettingRow title="Picture size" stack>
              <Slider label="Picture size" value={cfg.overlay.avatar_px} min={8} max={48} step={2} format={(v) => `${v} px`} onchange={(v) => set('overlay', { avatar_px: v })} />
              {#if cfg.overlay.avatar_px < 20 && cfg.overlay.style !== 'count'}
                <div class="hint" in:unfold out:fold>
                  <span>At this size, Count is easier to read.</span>
                  <Button small onclick={() => set('overlay', { style: 'count' })}>Use Count</Button>
                </div>
              {/if}
            </SettingRow>
            <SettingRow title="Show up to" stack>
              <Slider label="Show up to" value={cfg.overlay.max_shown} min={1} max={8} step={1} format={(v) => (v === 1 ? '1 person' : `${v} people`)} onchange={(v) => set('overlay', { max_shown: v })} />
            </SettingRow>
            <SettingRow title="Opacity" stack>
              <Slider label="Opacity" value={cfg.overlay.opacity} min={0.3} max={1} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onchange={(v) => set('overlay', { opacity: v })} />
            </SettingRow>
            <SettingRow title="Layout">
              <Segmented
                label="Layout"
                value={cfg.overlay.layout}
                options={[
                  { value: 'row', label: 'Row' },
                  { value: 'column', label: 'Column' },
                  { value: 'grid', label: 'Grid' },
                ]}
                onchange={(v) => set('overlay', { layout: v })}
              />
            </SettingRow>
            <SettingRow title="Hover shows">
              <Segmented
                label="Hover shows"
                value={cfg.overlay.bubble}
                options={[
                  { value: 'username', label: 'Username' },
                  { value: 'label', label: 'Name' },
                  { value: 'both', label: 'Both' },
                ]}
                onchange={(v) => set('overlay', { bubble: v })}
              />
            </SettingRow>
            <SettingRow title="Hide when nobody is there">
              <Switch label="Hide when nobody is there" checked={cfg.overlay.hide_when_empty} onchange={(v) => set('overlay', { hide_when_empty: v })} />
            </SettingRow>
            {#if snap.displays.length > 1}
              <SettingRow title="Display">
                <select class="select" aria-label="Display" value={cfg.overlay.display} onchange={(e) => set('overlay', { display: (e.target as HTMLSelectElement).value })}>
                  <option value="primary">Main display</option>
                  {#each snap.displays.filter((d) => !d.primary) as d}
                    <option value={d.name}>{d.label}</option>
                  {/each}
                </select>
              </SettingRow>
            {/if}
            <SettingRow
              title="Hide from screen sharing"
              sub="VC Alert's windows stay out of screen shares and recordings. {mac ? 'Some newer macOS capture tools may still show them.' : 'Needs Windows 10 version 2004 or later.'}"
            >
              <Switch label="Hide from screen sharing" checked={cfg.overlay.hide_from_capture} onchange={(v) => set('overlay', { hide_from_capture: v })} />
            </SettingRow>
          </div>
        {:else if tab === 'general'}
          <h1 id="h-general">General</h1>
          <div class="rows">
            <SettingRow title="Start watching when VC Alert opens">
              <Switch label="Start watching when VC Alert opens" checked={cfg.general.start_watching_on_launch} onchange={(v) => set('general', { start_watching_on_launch: v })} />
            </SettingRow>
            <SettingRow title="Open at login">
              <Switch label="Open at login" checked={cfg.general.launch_at_login} onchange={(v) => set('general', { launch_at_login: v })} />
            </SettingRow>
            <SettingRow
              title="Start in the background"
              sub={cfg.general.launch_at_login
                ? `When VC Alert opens at login, it waits ${mac ? 'in the menu bar' : 'in the notification area'} instead of opening its window. Opening it yourself always shows the window.`
                : 'Only applies when VC Alert opens at login. Turn on Open at login to use it.'}
            >
              <Switch
                label="Start in the background"
                checked={cfg.general.start_minimized}
                disabled={!cfg.general.launch_at_login}
                onchange={(v) => set('general', { start_minimized: v })}
              />
            </SettingRow>
            <SettingRow title="Keep running when the window is closed" sub="VC Alert stays {mac ? 'in the menu bar' : 'in the notification area'} and keeps watching.">
              <Switch label="Keep running when the window is closed" checked={cfg.general.close_to_tray} onchange={(v) => set('general', { close_to_tray: v })} />
            </SettingRow>
            {#if mac}
              <SettingRow title="Hide the Dock icon" sub="VC Alert lives only in the menu bar.">
                <Switch label="Hide the Dock icon" checked={cfg.general.hide_dock_icon} onchange={(v) => set('general', { hide_dock_icon: v })} />
              </SettingRow>
            {/if}
            <SettingRow title="Hide usernames in the list" sub="Shows mo•••••or instead, in case someone glances at your screen.">
              <Switch label="Hide usernames in the list" checked={cfg.general.mask_usernames} onchange={(v) => set('general', { mask_usernames: v })} />
            </SettingRow>
            <SettingRow title="Quiet mode" sub="No sound and no overlay. The list still shows who joins.">
              <Switch label="Quiet mode" checked={cfg.general.quiet} onchange={(v) => set('general', { quiet: v })} />
            </SettingRow>
            <SettingRow title="Show a reminder when Quiet is on" sub="A line under the status: “Quiet: no sound or overlay”.">
              <Switch
                label="Show a reminder when Quiet is on"
                checked={cfg.general.quiet_reminder}
                onchange={(v) => set('general', { quiet_reminder: v })}
              />
            </SettingRow>
          </div>
          <h2>Appearance</h2>
          <div class="rows">
            <SettingRow title="Theme">
              <Segmented
                label="Theme"
                value={themeValue}
                options={[
                  { value: 'onyx', label: 'Ink' },
                  { value: 'graphite', label: 'Graphite' },
                  { value: 'paper', label: 'Paper' },
                  { value: 'system', label: 'Match system' },
                ]}
                onchange={(v) => set('appearance', { theme: v })}
              />
            </SettingRow>
            <SettingRow title="List spacing">
              <Segmented
                label="List spacing"
                value={cfg.appearance.density}
                options={[
                  { value: 'compact', label: 'Compact' },
                  { value: 'comfortable', label: 'Comfortable' },
                ]}
                onchange={(v) => set('appearance', { density: v })}
              />
            </SettingRow>
          </div>
        {:else if tab === 'connection'}
          <h1 id="h-connection">Connection</h1>
          <p class="lead">Your own Discord application lets VC Alert talk to the Discord app on this computer. Changes apply the next time you start watching.</p>
          <CredentialsForm />
          <div class="rows">
            <SettingRow title="Discord approval" sub="Forget the approval you gave in Discord. It asks again the next time you start watching.">
              <Button small swap={resetDone ? 'done' : 'reset'} onclick={resetLogin}>{resetDone ? 'Forgotten' : 'Forget'}</Button>
            </SettingRow>
          </div>
        {:else if tab === 'data'}
          <h1 id="h-data">Import and export</h1>
          <div class="rows">
            <SettingRow title="Import a list" sub="A .json from a friend, a backup, or a plain list of usernames.">
              <Button small onclick={() => (pasting = true)}>Paste…</Button>
              <Button small onclick={() => importFile()}>Choose file…</Button>
            </SettingRow>
            <SettingRow title="Export" sub="Share your list with friends, or keep a full backup.">
              <Button small kind="primary" onclick={() => (exporting = true)}>Export…</Button>
            </SettingRow>
          </div>
          <h2>Backups</h2>
          <p class="lead">Saved automatically before every import or restore. The last 10 are kept.</p>
          {#if backups.length}
            <ul class="backups">
              {#each backups as b (b.name)}
                <li>
                  <Icon name="history" size={16} />
                  <span class="num">{b.when}</span>
                  <Button
                    small
                    kind="ghost"
                    onclick={() =>
                      call('restore_backup', { name: b.name })
                        .then(() => (toast('ok', 'Restored. Your list and settings are back to that moment.'), loadBackups()))
                        .catch((e) => toast('error', errText(e)))}>Restore</Button
                  >
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty">No backups yet.</p>
          {/if}
          <h2>Storage</h2>
          <div class="rows">
            <SettingRow title="Saved profile pictures" sub="Kept so pictures show up straight away. They download again when someone joins.">
              <Button small onclick={() => call<number>('clear_pictures').then((n) => toast('ok', n === 1 ? 'Removed 1 saved file.' : `Removed ${n} saved files.`))}>Clear</Button>
            </SettingRow>
            <SettingRow title="Data folder" sub={snap.data_dir}>
              <Button small onclick={() => call('reveal_data_folder')}>Show</Button>
            </SettingRow>
          </div>
        {:else}
          <h1 id="h-about">About</h1>
          <p class="lead">VC Alert {snap.version}</p>
          <p class="body">VC Alert talks only to the Discord app on this computer, to Discord's sign-in when you approve it, and to Discord's picture server for profile pictures. No bot, no account token, nothing posted anywhere.</p>
          <p class="body">Your Client Secret and Discord approval are kept in your system's keychain.</p>
        {/if}
      </section>
    {/key}
  </main>
</div>

{#if pasting}
  <PasteList onclose={() => (pasting = false)} onjson={(text) => ((pasting = false), (importing = { text }))} />
{/if}
{#if importing}
  <ImportSheet source={importing} onclose={() => (importing = null)} />
{/if}
{#if exporting}
  <ExportSheet onclose={() => (exporting = false)} />
{/if}
<Toasts />

<style>
  .settings {
    display: grid;
    grid-template-columns: 188px minmax(0, 1fr);
    height: 100%;
  }
  nav {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--sp-0);
    padding: var(--sp-4) var(--sp-2);
    border-right: 1px solid var(--line);
    background: var(--surface);
  }
  .hl {
    position: absolute;
    z-index: 0;
    opacity: 0;
    border-radius: var(--r-sm);
    background: var(--accent-wash);
    transform-origin: 0 0;
    pointer-events: none;
  }
  nav button {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    height: var(--h-md);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    text-align: left;
    transition:
      color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  nav button:hover {
    color: var(--text);
  }
  nav button:active {
    transform: scale(var(--press-scale));
  }
  nav button[aria-current='page'] {
    color: var(--accent-text);
    font-weight: var(--fw-medium);
  }
  .saves {
    margin: auto var(--sp-2) 0;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  main {
    overflow-y: auto;
    padding: var(--sp-5) var(--sp-6) var(--sp-8);
  }
  section {
    max-width: 560px;
  }
  h1 {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-xl);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  h2 {
    margin: var(--sp-6) 0 var(--sp-1);
    font-size: var(--fs-md);
    font-weight: var(--fw-bold);
  }
  h1 + h2 {
    margin-top: 0;
  }
  .lead {
    margin: 0 0 var(--sp-3);
    max-width: 60ch;
    color: var(--text-2);
  }
  .body {
    margin: 0 0 var(--sp-3);
    max-width: 60ch;
    color: var(--text-2);
  }
  .rows {
    margin-top: var(--sp-3);
  }
  .hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .select {
    height: var(--h-md);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: var(--raised);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--text);
  }
  .backups {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .backups li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--h-lg);
    color: var(--text-2);
    border-bottom: 1px solid var(--line);
  }
  .backups li span {
    flex: 1;
  }
  .empty {
    color: var(--text-3);
  }
  @media (prefers-reduced-motion: reduce) {
    nav button:active {
      transform: none;
    }
  }
</style>
