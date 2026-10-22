<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Button from '../components/Button.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Icon from '../components/Icon.svelte';
  import Lamp from '../components/Lamp.svelte';
  import Menu, { type MenuItem } from '../components/Menu.svelte';
  import RollingNumber from '../components/RollingNumber.svelte';
  import SpeakerIcon from '../components/SpeakerIcon.svelte';
  import TextField from '../components/TextField.svelte';
  import Toasts from '../components/Toasts.svelte';
  import PersonRow from './PersonRow.svelte';
  import AddPerson from '../sheets/AddPerson.svelte';
  import PersonSound from '../sheets/PersonSound.svelte';
  import ImportSheet from '../sheets/ImportSheet.svelte';
  import PasteList from '../sheets/PasteList.svelte';
  import Wizard from '../sheets/Wizard.svelte';
  import { app, avatars, loadAvatars, toast, watchAvatars } from '../lib/app.svelte';
  import { call, errText, on, onFileDrop, pickFile } from '../lib/api';
  import { flipParams, glide, rise, shake, shrink, swapIn, unfold, fold } from '../lib/motion';
  import { describeBeep, baseName, keyOf, lockMs, matchesQuery, parseIdent, sortTargets } from '../lib/util';
  import type { Target } from '../lib/types';

  const snap = $derived(app.snap!);
  const cfg = $derived(snap.config);
  const n = $derived(cfg.targets.length);
  const quiet = $derived(cfg.general.quiet);

  // ------------------------------------------------------------ status --
  let starting = $state(false);
  let joinedMsg = $state<string | null>(null);
  let joinedTimer: ReturnType<typeof setTimeout> | undefined;
  let flashKey = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let announce = $state('');

  const inCall = $derived(snap.watching ? snap.members.length : 0);

  // One row, as in the early mockup: a status pill (lamp, state, channel,
  // how many are here), then Quiet, Settings and Start/Stop. Anything longer
  // (what went wrong, what to do next, Quiet) unfolds on a line below.
  type Status = {
    tone: 'off' | 'warn' | 'live' | 'danger';
    head: string;
    channel?: string;
    count?: boolean;
    sub?: string;
    tip: string;
    action: 'start' | 'stop' | 'fix';
  };

  const status = $derived.by<Status>(() => {
    const problem = (detail: string): Status => {
      const sub = detail || 'Check your Discord app details.';
      return { tone: 'danger', head: 'Setup problem', sub, tip: `Setup problem: ${sub}`, action: 'fix' };
    };
    if (!snap.watching) {
      if (snap.status === 'setup_error') return problem(snap.detail);
      return { tone: 'off', head: 'Not watching', tip: `Not watching. ${n === 1 ? '1 person' : `${n} people`} on your list.`, action: 'start' };
    }
    switch (snap.status) {
      case 'idle_no_channel':
        return { tone: 'warn', head: 'Connected', sub: 'Join a voice channel to start.', tip: 'Connected to Discord, not in a voice channel', action: 'stop' };
      case 'watching_channel': {
        const ch = snap.detail || 'your channel';
        return { tone: 'live', head: 'Watching', channel: ch, count: true, tip: `Watching ${ch}`, action: 'stop' };
      }
      case 'reconnecting': {
        const d = snap.detail ?? '';
        const sub = d.includes('Could not reach')
          ? 'Open the Discord desktop app. VC Alert keeps trying every 5 seconds.'
          : d.length > 60
            ? d.slice(0, 60) + '…'
            : d || 'Lost the connection to Discord.';
        return { tone: 'warn', head: 'Reconnecting…', sub, tip: `Reconnecting: ${sub}`, action: 'stop' };
      }
      case 'setup_error':
        return problem(snap.detail);
      default:
        return { tone: 'warn', head: 'Connecting to Discord…', tip: 'Connecting to Discord', action: 'stop' };
    }
  });
  const pillTip = $derived(
    status.count ? `${status.tip}. ${inCall === 1 ? '1 of your list is' : `${inCall} of your list are`} in the call.` : status.tip,
  );

  const buttonLabel = $derived(
    snap.stopping ? 'Stopping…' : starting ? 'Connecting…' : status.action === 'start' ? 'Start watching' : status.action === 'fix' ? 'Fix in Settings' : 'Stop',
  );

  async function primary() {
    if (status.action === 'fix') return call('open_settings', { tab: 'connection' });
    if (status.action === 'stop') return call('stop_watching');
    starting = true;
    try {
      await call('start_watching');
    } catch (e) {
      const msg = errText(e);
      if (msg === 'needs-credentials') call('open_settings', { tab: 'connection' });
      else toast('error', msg, { ms: 6000 });
    } finally {
      starting = false;
    }
  }

  // --------------------------------------------------- join times (UI) --
  let joinedAt = $state<Record<string, number>>({});
  let now = $state(Date.now());
  $effect(() => {
    const keys = new Set(snap.watching ? snap.members.map((m) => m.key.toLowerCase()) : []);
    const next: Record<string, number> = {};
    for (const k of keys) next[k] = joinedAt[k] ?? Date.now();
    if (Object.keys(next).length !== Object.keys(joinedAt).length || Object.keys(next).some((k) => !(k in joinedAt))) joinedAt = next;
  });
  const since = (key: string) => {
    const t = joinedAt[key.toLowerCase()];
    if (!t) return null;
    const m = Math.floor((now - t) / 60000);
    return m < 1 ? 'now' : m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
  };

  // ------------------------------------------------------------- search --
  let query = $state('');
  // Filtering while typing doesn't animate; adding, removing and reordering do.
  let settled = $state('');
  $effect(() => {
    const now = query;
    requestAnimationFrame(() => (settled = now));
  });
  const filtering = $derived(query !== settled);
  let searchEl: HTMLInputElement | undefined = $state();
  let searchBox: HTMLDivElement | undefined = $state();
  let hintShake = $state(false);

  const q = $derived(query.trim());
  const tokens = $derived(q.split(/[\s,;]+/).filter(Boolean));
  // A pasted list needs at least two real usernames/IDs; otherwise it's just bad input.
  const multi = $derived(tokens.length > 1 && tokens.filter((t) => parseIdent(t)).length >= 2);
  const matches = $derived(q && !multi && tokens.length < 2 ? cfg.targets.filter((t) => matchesQuery(t, q)) : q ? [] : cfg.targets);
  const single = $derived(!multi && q && tokens.length === 1 ? parseIdent(q) : null);
  const bulk = $derived.by(() => {
    if (!multi) return [] as string[];
    const seen = new Set<string>();
    return tokens.filter((tok) => {
      const p = parseIdent(tok);
      const k = p?.id ?? p?.username;
      if (!k || seen.has(k) || cfg.targets.some((t) => keyOf(t).toLowerCase() === k)) return false;
      seen.add(k);
      return true;
    });
  });
  const addRow = $derived.by(() => {
    if (!q || matches.length) return null;
    if (multi) return bulk.length ? { kind: 'many' as const, label: bulk.length === 1 ? 'Add 1 person' : `Add ${bulk.length} people` } : null;
    if (single) return { kind: 'one' as const, name: single.username ? `@${single.username}` : `ID ${single.id}` };
    return null;
  });
  const showHint = $derived(!!q && !matches.length && !addRow);
  const shortcut = $derived(snap.platform === 'macos' ? '⌘F' : 'Ctrl F');

  async function addFromSearch() {
    if (!addRow) {
      if (showHint) {
        hintShake = true;
        shake(searchBox);
        announce = 'Usernames use letters, numbers, _ and . (2 to 32 characters), or paste a numeric user ID.';
      }
      return;
    }
    try {
      if (addRow.kind === 'one') {
        const t = await call<Target>('add_target', { label: '', ident: q });
        const k = keyOf(t);
        query = '';
        await revealAndFocus(k);
        toast('ok', `Added ${t.username ? '@' + t.username : t.label}`, {
          action: { label: 'Undo', run: () => call('remove_target', { key: k }) },
        });
      } else {
        const r = await call<{ added: Target[]; skipped: { value: string }[] }>('add_many', { text: query });
        query = '';
        const keys = r.added.map(keyOf);
        const msg = `Added ${keys.length === 1 ? '1 person' : `${keys.length} people`}${r.skipped.length ? `, skipped ${r.skipped.length}` : ''}`;
        toast('ok', msg, { action: keys.length ? { label: 'Undo', run: () => keys.forEach((key) => call('remove_target', { key })) } : undefined });
      }
    } catch (e) {
      toast('error', errText(e));
      shake(searchBox);
    }
  }

  // --------------------------------------------------------------- list --
  const presentKeys = $derived(new Set(snap.watching ? snap.members.map((m) => m.key.toLowerCase()) : []));
  const isPresent = (t: Target) => presentKeys.has(keyOf(t).toLowerCase());

  type Item = { kind: 'head'; id: string; text: string; count: number; live: boolean } | { kind: 'row'; id: string; t: Target; present: boolean };

  const items = $derived.by<Item[]>(() => {
    const sorted = sortTargets(matches);
    const live = sorted.filter(isPresent);
    const rest = sorted.filter((t) => !isPresent(t));
    const out: Item[] = [];
    if (live.length) {
      out.push({ kind: 'head', id: 'h-live', text: 'In your channel', count: live.length, live: true });
      live.forEach((t) => out.push({ kind: 'row', id: 'r-' + keyOf(t), t, present: true }));
      if (rest.length) out.push({ kind: 'head', id: 'h-rest', text: 'Everyone else', count: rest.length, live: false });
    } else if (rest.length && (q || n)) {
      out.push({ kind: 'head', id: 'h-rest', text: q ? 'Matches' : 'Watch list', count: rest.length, live: false });
    }
    rest.forEach((t) => out.push({ kind: 'row', id: 'r-' + keyOf(t), t, present: false }));
    return out;
  });
  const rowKeys = $derived(items.filter((i) => i.kind === 'row').map((i) => (i as { t: Target }).t).map(keyOf));

  let activeKey = $state<string | null>(null);
  const activeRow = $derived(activeKey && rowKeys.includes(activeKey) ? activeKey : (rowKeys[0] ?? null));
  let rows: Record<string, PersonRow> = $state({});
  let listEl: HTMLDivElement | undefined = $state();
  let edgeTop = $state(false);
  let edgeBottom = $state(false);

  function edges() {
    if (!listEl) return;
    edgeTop = listEl.scrollTop > 2;
    edgeBottom = listEl.scrollTop + listEl.clientHeight < listEl.scrollHeight - 2;
  }
  $effect(() => {
    void items.length;
    tick().then(edges);
  });

  function move(from: string, dir: 1 | -1 | 'first' | 'last') {
    const i = rowKeys.indexOf(from);
    const j = dir === 'first' ? 0 : dir === 'last' ? rowKeys.length - 1 : Math.min(rowKeys.length - 1, Math.max(0, i + dir));
    const k = rowKeys[j];
    if (k) {
      activeKey = k;
      rows[k]?.focus();
    }
  }

  async function revealAndFocus(key: string) {
    await tick();
    activeKey = key;
    await tick();
    rows[key]?.focus();
  }

  // --------------------------------------------------------- row actions --
  let menu = $state<{ x: number; y: number; items: MenuItem[]; label: string } | null>(null);
  let renaming = $state<string | null>(null);
  let soundFor = $state<Target | null>(null);

  function rowMenu(t: Target, x: number, y: number) {
    const k = keyOf(t);
    const handle = t.username ? '@' + t.username : t.id!;
    menu = {
      x,
      y,
      label: `Actions for ${t.label}`,
      items: [
        { label: 'Rename', icon: 'edit', run: () => (renaming = k) },
        { label: 'Sound…', icon: 'volume_up', run: () => (soundFor = t) },
        {
          label: t.username ? 'Copy username' : 'Copy user ID',
          icon: 'content_copy',
          run: async () => {
            await navigator.clipboard.writeText(handle);
            toast('ok', `Copied ${handle}`);
          },
        },
        { label: 'Remove', icon: 'delete', danger: true, run: () => remove(t) },
      ],
    };
  }

  async function remove(t: Target) {
    const k = keyOf(t);
    const i = rowKeys.indexOf(k);
    const next = rowKeys[i + 1] ?? rowKeys[i - 1] ?? null;
    try {
      const r = await call<{ id: number; label: string } | null>('remove_target', { key: k });
      if (!r) return;
      if (next) revealAndFocus(next);
      toast('info', `Removed “${r.label}”`, {
        ms: 7000,
        action: { label: 'Undo', run: async () => (await call('undo_remove', { id: r.id }), revealAndFocus(k)) },
      });
    } catch (e) {
      toast('error', errText(e));
    }
  }

  async function rename(t: Target, label: string | null) {
    const k = keyOf(t);
    if (label === null) {
      renaming = k;
      return;
    }
    renaming = null;
    if (label.trim() !== t.label) await call('rename_target', { key: k, label });
    revealAndFocus(k);
  }

  // ------------------------------------------------------------- footer --
  let testing = $state(false);
  async function test() {
    if (testing) return;
    try {
      const d = await call<number>('test_sound', { spec: null });
      testing = true;
      setTimeout(() => (testing = false), lockMs(d));
    } catch (e) {
      toast('error', errText(e));
    }
  }

  // -------------------------------------------------------------- sheets --
  let sheet = $state<'add' | 'paste' | null>(null);
  let importing = $state<{ path?: string; text?: string } | null>(null);
  let dropping = $state(false);

  function addMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = {
      x: r.right,
      y: r.bottom + 4,
      label: 'Add people',
      items: [
        { label: 'Add with a name…', icon: 'person_add', run: () => (sheet = 'add') },
        { label: 'Paste a list…', icon: 'content_paste', run: () => (sheet = 'paste') },
        {
          label: 'Import a file…',
          icon: 'file_open',
          run: async () => {
            const p = await pickFile('Import a list', [{ name: 'JSON', extensions: ['json'] }]);
            if (p) importing = { path: p };
          },
        },
      ],
    };
  }

  const setupMissing = $derived(cfg.general.setup_complete && (!cfg.client_id || !snap.has_secret));

  onMount(() => {
    loadAvatars(cfg.targets.map(keyOf));
    watchAvatars();
    const timer = setInterval(() => (now = Date.now()), 30000);
    const offs: Promise<() => void>[] = [
      on<{ key: string; label: string }>('alert', ({ key, label }) => {
        clearTimeout(joinedTimer);
        clearTimeout(flashTimer);
        joinedMsg = `${label} joined`;
        announce = `${label} joined`;
        flashKey = null;
        tick().then(() => (flashKey = key.toLowerCase()));
        joinedTimer = setTimeout(() => (joinedMsg = null), 4000);
        flashTimer = setTimeout(() => (flashKey = null), 1600);
      }),
      on<{ kind: 'ok' | 'error' | 'info'; message: string }>('toast', ({ kind, message }) =>
        toast(kind, message, { ms: kind === 'error' ? 9000 : 5000 }),
      ),
      on<string>('import-file', (path) => (importing = { path })),
      onFileDrop((kind, paths) => {
        if (kind === 'over') dropping = true;
        else if (kind === 'leave') dropping = false;
        else {
          dropping = false;
          const p = paths.find((x) => x.toLowerCase().endsWith('.json'));
          if (p) importing = { path: p };
          else toast('error', 'Drop a .json list or backup to import it.');
        }
      }),
    ];
    return () => {
      clearInterval(timer);
      offs.forEach((p) => p.then((off) => off()));
    };
  });

  $effect(() => {
    loadAvatars(cfg.targets.map(keyOf));
  });

  function globalKeys(e: KeyboardEvent) {
    const mod = snap.platform === 'macos' ? e.metaKey : e.ctrlKey;
    if (mod && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
    } else if (mod && e.key === ',') {
      e.preventDefault();
      call('open_settings', { tab: null });
    } else if (mod && e.key === 'Enter') {
      e.preventDefault();
      primary();
    }
  }
</script>

<svelte:window onkeydown={globalKeys} />

<div class="main">
  <header class="status">
    <div class="bar">
      <div class="pill" title={pillTip}>
        <Lamp tone={status.tone} ping={status.tone === 'live'} />
        <span class="pill-text">
          {#key joinedMsg ?? status.head}
            <span class="lead" class:joined={!!joinedMsg} in:swapIn>{joinedMsg ?? status.head}</span>
          {/key}
          {#if status.channel && !joinedMsg}
            <span class="sep" aria-hidden="true">·</span><span class="channel">{status.channel}</span>
          {/if}
          {#if status.count}
            <span class="sep" aria-hidden="true">·</span><span class="here" class:live={inCall > 0}><RollingNumber value={inCall} /> here</span>
          {/if}
        </span>
      </div>
      <div class="tools">
        <IconButton
          round
          flat={false}
          icon={quiet ? 'notifications_off' : 'notifications'}
          label={quiet ? 'Quiet is on. Turn off' : 'Quiet mode: no sound and no overlay'}
          active={quiet}
          onclick={() => call('toggle_quiet')}
        />
        <IconButton round flat={false} icon="settings" label="Settings" onclick={() => call('open_settings', { tab: null })} />
        <Button
          kind={status.action === 'start' ? 'primary' : status.action === 'fix' ? 'primary' : 'secondary'}
          swap={buttonLabel}
          disabled={snap.stopping}
          onclick={primary}>{buttonLabel}</Button
        >
      </div>
    </div>
    {#if status.sub}
      <div class="detail" in:unfold out:fold>
        {#key status.sub}<p class="detail-in sub" in:swapIn>{status.sub}</p>{/key}
      </div>
    {/if}
    {#if quiet}
      <div class="detail" in:unfold out:fold>
        <div class="detail-in quiet">Quiet: no sound or overlay <Button kind="link" small onclick={() => call('toggle_quiet')}>Turn off</Button></div>
      </div>
    {/if}
  </header>

  {#if setupMissing}
    <div class="banner" in:unfold out:fold>
      <div class="banner-in">
        <Icon name="info" size={16} />
        <span>Add your Discord app details so VC Alert can talk to Discord.</span>
        <Button small kind="primary" onclick={() => call('open_settings', { tab: 'connection' })}>Set up</Button>
      </div>
    </div>
  {/if}
  {#if snap.notice}
    <div class="banner warn" in:unfold out:fold>
      <div class="banner-in">
        <Icon name="warning" size={16} />
        <span>{snap.notice}</span>
        <Button small onclick={() => call('dismiss_notice')}>Dismiss</Button>
      </div>
    </div>
  {/if}

  <div class="search">
    <div class="field" bind:this={searchBox}>
      <TextField
        bind:value={query}
        bind:input={searchEl}
        icon="search"
        label="Search your list or add a username"
        placeholder="Search or add a username"
        invalid={hintShake && showHint}
        describedby="search-hint"
        oninput={() => (hintShake = false)}
        onenter={addFromSearch}
        onescape={() => (query = '')}
        onkeydown={(e) => {
          if (e.key === 'ArrowDown' && rowKeys.length) {
            e.preventDefault();
            move(rowKeys[0], 'first');
          }
        }}
      >
        {#snippet trailing()}
          <kbd>{q ? 'Esc' : shortcut}</kbd>
        {/snippet}
      </TextField>
    </div>
    <IconButton icon="person_add" label="Add people" onclick={addMenu} />
  </div>
  {#if showHint}
    <p class="hint" id="search-hint" in:unfold out:fold>Usernames use letters, numbers, _ and . (2 to 32 characters), or paste a numeric user ID.</p>
  {/if}

  <div
    class="list"
    class:edge-top={edgeTop}
    class:edge-bottom={edgeBottom}
    bind:this={listEl}
    onscroll={edges}
    role="list"
    aria-label="Watch list"
  >
    {#if addRow}
      <button type="button" class="add-row" in:rise out:shrink onclick={addFromSearch}>
        <span class="add-ic"><Icon name="person_add" size={18} /></span>
        <span class="add-text">
          {#if addRow.kind === 'one'}
            <span>Add <span class="id">{addRow.name}</span></span>
          {:else}
            <span>{addRow.label}</span>
          {/if}
          <span class="add-sub">Press Enter</span>
        </span>
      </button>
    {/if}

    {#each items as it (it.id)}
      <div class="item" class:is-head={it.kind === 'head'} class:is-row={it.kind === 'row'} animate:glide={flipParams(filtering)} in:rise={{ skip: filtering }} out:shrink={{ skip: filtering }}>
        {#if it.kind === 'head'}
          <div class="sec" class:live={it.live} role="presentation">{it.text} <span class="count"><RollingNumber value={it.count} /></span></div>
        {:else}
          {@const k = keyOf(it.t)}
          <PersonRow
            bind:this={rows[k]}
            t={it.t}
            present={it.present}
            since={it.present ? since(k) : null}
            masked={cfg.general.mask_usernames}
            src={avatars[k] ?? null}
            joined={flashKey === k.toLowerCase()}
            active={activeRow === k}
            renaming={renaming === k}
            onmenu={(x, y) => rowMenu(it.t, x, y)}
            onsound={() => (soundFor = it.t)}
            onremove={() => remove(it.t)}
            onrename={(l) => rename(it.t, l)}
            onfocusrow={() => (activeKey = k)}
            onmove={(d) => move(k, d)}
          />
        {/if}
      </div>
    {/each}

    {#if !n}
      <div class="empty" in:rise>
        <p class="empty-head">Add the first person to watch</p>
        <p class="empty-sub">Type their Discord username above and press Enter, or import a list from the add menu.</p>
      </div>
    {:else if q && !matches.length && !addRow && !showHint}
      <div class="empty" in:rise>
        <p class="empty-head">Everyone in that list is already here</p>
      </div>
    {/if}
  </div>

  <footer>
    <div class="sound">
      <span class="spk"><SpeakerIcon size={18} playing={testing} /></span>
      <button type="button" class="sound-text" onclick={() => call('open_settings', { tab: 'sound' })} title="Change the universal sound">
        <span class="sound-name">Universal sound</span>
        <span class="sound-desc num">{cfg.sound_file ? baseName(cfg.sound_file) : describeBeep(cfg.beep)}</span>
      </button>
      <Button small onclick={test} label="Test the universal sound"><Icon name="play_arrow" size={16} /> Test</Button>
    </div>
    {#if !cfg.overlay.enabled}
      <div class="overlay-off" in:unfold out:fold>
        <div class="overlay-off-in">
          Overlay is off. <Button kind="link" small onclick={() => call('update_settings', { patch: { overlay: { enabled: true } } })}>Turn on</Button>
        </div>
      </div>
    {/if}
  </footer>

  <div class="sr-only" role="status" aria-live="polite">{announce}</div>

  {#if dropping}
    <div class="drop" in:swapIn out:fold>
      <Icon name="file_open" size={28} />
      <span>Drop a .json file to import it</span>
    </div>
  {/if}
</div>

{#if menu}
  <Menu x={menu.x} y={menu.y} items={menu.items} label={menu.label} onclose={() => (menu = null)} />
{/if}
{#if sheet === 'add'}
  <AddPerson
    onclose={() => (sheet = null)}
    onadded={(t) => {
      sheet = null;
      revealAndFocus(keyOf(t));
    }}
  />
{/if}
{#if sheet === 'paste'}
  <PasteList
    onclose={() => (sheet = null)}
    onjson={(text) => {
      sheet = null;
      importing = { text };
    }}
  />
{/if}
{#if soundFor}
  <PersonSound t={soundFor} onclose={() => (soundFor = null)} />
{/if}
{#if !cfg.general.setup_complete && !importing}
  <Wizard />
{/if}
{#if importing}
  <ImportSheet source={importing} onclose={() => (importing = null)} />
{/if}
<Toasts bottom={cfg.overlay.enabled ? 64 : 92} />

<style>
  .main {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 380px;
  }

  /* ---------------------------------------------------------- status */
  .status {
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .pill {
    display: flex;
    flex: 0 1 auto;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    height: var(--h-md);
    padding: 0 var(--sp-3) 0 var(--sp-2);
    border-radius: var(--r-full);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .pill-text {
    display: flex;
    align-items: baseline;
    gap: var(--sp-1);
    min-width: 0;
    white-space: nowrap;
  }
  /* Only the channel name (full name in the tooltip) or a long "… joined"
     gives way; the state word and the count always stay. */
  .lead,
  .channel {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lead {
    flex: none;
    font-weight: var(--fw-medium);
  }
  .lead.joined {
    flex: 0 1 auto;
    color: var(--live-text);
  }
  .channel {
    flex: 0 1 auto;
    color: var(--text-2);
  }
  .sep {
    flex: none;
    color: var(--text-3);
  }
  .here {
    flex: none;
    color: var(--text-2);
  }
  .here.live {
    color: var(--live-text);
    font-weight: var(--fw-medium);
  }
  .tools {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-2);
    margin-left: auto;
  }
  .detail-in {
    margin: 0;
    padding-top: var(--sp-2);
  }
  .sub {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .quiet {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }

  /* --------------------------------------------------------- banners */
  .banner {
    border-bottom: 1px solid var(--line);
    background: var(--accent-wash);
  }
  .banner.warn {
    background: var(--warn-wash);
  }
  .banner-in {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-4);
    color: var(--text);
    font-size: var(--fs-sm);
  }
  .banner-in > span {
    flex: 1;
    min-width: 0;
  }
  .banner :global(.icon) {
    color: var(--accent-text);
  }
  .banner.warn :global(.icon) {
    color: var(--warn);
  }

  /* ---------------------------------------------------------- search */
  .search {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    padding: var(--sp-3) var(--sp-4) var(--sp-1);
  }
  .field {
    flex: 1;
    min-width: 0;
  }
  kbd {
    margin-right: var(--sp-2);
    padding: 0 var(--sp-1);
    border-radius: var(--r-xs);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--text-3);
    font-family: var(--font-ui);
    font-size: var(--fs-xs);
    line-height: 18px;
  }
  .hint {
    margin: 0;
    padding: 0 var(--sp-4) var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }

  /* ------------------------------------------------------------ list */
  .list {
    position: relative;
    flex: 1;
    min-height: 96px;
    overflow-y: auto;
    padding: var(--sp-1) var(--sp-2) var(--sp-3);
    --fade: 24px;
    /* Keyboard focus never lands inside the faded edges. */
    scroll-padding-block: var(--fade);
  }
  .list.edge-top {
    mask-image: linear-gradient(to bottom, transparent, currentColor var(--fade));
  }
  .list.edge-bottom {
    mask-image: linear-gradient(to top, transparent, currentColor var(--fade));
  }
  .list.edge-top.edge-bottom {
    mask-image: linear-gradient(to bottom, transparent, currentColor var(--fade), currentColor calc(100% - var(--fade)), transparent);
  }
  /* Rows are opaque and sit above headings, so a row gliding past a heading
     covers it instead of mixing text. */
  .item {
    position: relative;
    z-index: 1;
    border-radius: var(--r-md);
    background: var(--bg);
  }
  .item.is-head {
    z-index: 0;
    background: none;
  }
  /* A grey hairline between people (never under a heading), lined up with the
     names. It fades beside a hovered or focused row, whose highlight takes over. */
  .item.is-row + .item.is-row::before {
    content: '';
    position: absolute;
    top: 0;
    left: calc(var(--sp-2) + var(--avatar) + var(--sp-3));
    right: var(--sp-2);
    height: 1px;
    background: var(--line);
    pointer-events: none;
    transition: opacity var(--t-fade) var(--ease-color);
  }
  .item.is-row:hover::before,
  .item.is-row:focus-within::before,
  .item.is-row:hover + .item.is-row::before,
  .item.is-row:focus-within + .item.is-row::before {
    opacity: 0;
  }
  .sec {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-2) var(--sp-1);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  .sec.live {
    color: var(--live-text);
  }
  .count {
    color: var(--text-3);
    font-weight: var(--fw-regular);
  }
  .add-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    margin-top: var(--sp-1);
    padding: var(--row-y) var(--sp-2);
    border: 0;
    border-radius: var(--r-md);
    background: var(--hover);
    color: var(--text);
    text-align: left;
    transition: transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .add-row:active {
    transform: scale(var(--row-press-scale));
  }
  .add-ic {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--avatar);
    height: var(--avatar);
    border-radius: var(--r-full);
    background: var(--accent-wash);
    color: var(--accent-text);
  }
  .add-text {
    display: grid;
    font-weight: var(--fw-medium);
  }
  .add-text .id {
    font-size: var(--fs-md);
  }
  .add-sub {
    color: var(--text-3);
    font-size: var(--fs-sm);
    font-weight: var(--fw-regular);
  }
  .empty {
    padding: var(--sp-8) var(--sp-4);
    text-align: center;
  }
  .empty-head {
    margin: 0;
    font-weight: var(--fw-medium);
  }
  .empty-sub {
    max-width: 34ch;
    margin: var(--sp-1) auto 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }

  /* ---------------------------------------------------------- footer */
  footer {
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .sound {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3) var(--sp-2) var(--sp-4);
  }
  .spk {
    display: flex;
    color: var(--text-2);
  }
  .sound-text {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: baseline;
    gap: var(--sp-2);
    height: var(--h-md);
    padding: 0 var(--sp-1);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    text-align: left;
    white-space: nowrap;
    transition: background-color var(--t-fade) var(--ease-color);
  }
  .sound-text:hover {
    background: var(--hover);
  }
  .sound-text {
    align-items: center;
  }
  .sound-name {
    flex: none;
    font-weight: var(--fw-medium);
  }
  .sound-desc {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .overlay-off-in {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    padding: 0 var(--sp-4) var(--sp-2);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }

  /* ------------------------------------------------------------ drop */
  .drop {
    position: absolute;
    inset: var(--sp-2);
    z-index: 40;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    border-radius: var(--r-lg);
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--accent-text);
    font-weight: var(--fw-medium);
  }

  @media (prefers-reduced-motion: reduce) {
    .add-row:active {
      transform: none;
    }
  }
</style>
