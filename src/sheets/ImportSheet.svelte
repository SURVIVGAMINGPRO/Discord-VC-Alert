<script lang="ts">
  // Shows what's in a file before anything changes, then merges or replaces.
  import Sheet from '../components/Sheet.svelte';
  import Button from '../components/Button.svelte';
  import Icon from '../components/Icon.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Switch from '../components/Switch.svelte';
  import { call, errText } from '../lib/api';
  import { toast } from '../lib/app.svelte';
  import { fold, swapIn, unfold } from '../lib/motion';
  import { baseName } from '../lib/util';
  import type { Preview } from '../lib/types';

  let { source, onclose }: { source: { path?: string; text?: string; migration?: boolean }; onclose: () => void } = $props();

  let preview = $state<Preview | null>(null);
  let error = $state('');
  let mode = $state<'merge' | 'replace'>('merge');
  let selected = $state<Record<string, boolean>>({});
  let theirs = $state<Record<string, boolean>>({});
  let importSettings = $state(false);
  let importCreds = $state(false);
  let showSkipped = $state(false);
  let busy = $state(false);

  $effect(() => {
    call<Preview>('import_preview', { path: source.path ?? null, text: source.text ?? null, migration: !!source.migration })
      .then((p) => {
        preview = p;
        for (const r of p.rows) selected[r.key] = r.status !== 'same';
        importSettings = !!source.migration && p.has_settings;
        importCreds = !!source.migration && p.has_credentials;
      })
      .catch((e) => (error = errText(e)));
  });

  const count = $derived(preview ? preview.rows.filter((r) => selected[r.key]).length : 0);
  const fileName = $derived(source.path ? baseName(source.path) : 'Pasted text');

  function close() {
    call('import_cancel');
    onclose();
  }

  async function apply() {
    if (!preview) return;
    busy = true;
    try {
      const r = await call<{ added: number; updated: number; removed: number; pictures: number }>('import_apply', {
        options: {
          mode,
          selected: preview.rows.filter((x) => selected[x.key]).map((x) => x.key),
          theirs: preview.rows.filter((x) => theirs[x.key]).map((x) => x.key),
          import_settings: importSettings,
          import_credentials: importCreds,
        },
      });
      const parts = [`${r.added} added`];
      if (r.updated) parts.push(`${r.updated} updated`);
      if (r.removed) parts.push(`${r.removed} removed`);
      toast('ok', `Imported: ${parts.join(', ')}. A backup was saved first.`);
      onclose();
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }

  const STATUS = { new: 'New', same: 'Already here', different: 'Different' } as const;
</script>

<Sheet title="Import a list" width={480} onclose={close}>
  {#if error && !preview}
    <p class="error" role="alert">{error}</p>
  {:else if !preview}
    <p class="muted">Reading {fileName}…</p>
  {:else}
    <div class="file">
      <Icon name="file_open" size={20} />
      <div class="file-text">
        <div class="file-name">{fileName}</div>
        <div class="file-kind">{preview.kind_label}{preview.name ? `: ${preview.name}` : ''}</div>
      </div>
    </div>

    <dl class="counts">
      <div><dt>New</dt><dd class="new num">{preview.new}</dd></div>
      <div><dt>Already here</dt><dd class="num">{preview.same}</dd></div>
      <div><dt>Different</dt><dd class="diff num">{preview.different}</dd></div>
    </dl>

    <div class="row">
      <Segmented
        label="How to import"
        value={mode}
        options={[
          { value: 'merge', label: 'Add to my list' },
          { value: 'replace', label: 'Replace my list' },
        ]}
        onchange={(v) => (mode = v)}
      />
    </div>
    {#if mode === 'replace'}
      <p class="note warn" in:unfold out:fold>People who aren't ticked below will be removed from your list. A backup is saved first.</p>
    {/if}

    <ul class="rows" aria-label="People in the file">
      {#each preview.rows as r (r.key)}
        <li class:dim={r.status === 'same'}>
          <label class="pick">
            <input type="checkbox" bind:checked={selected[r.key]} />
            <span class="who">
              <span class="name">{r.label}</span>
              {#if r.username && r.username !== r.label.toLowerCase()}<span class="id">@{r.username}</span>{/if}
              {#if r.id}<span class="id">ID {r.id.slice(0, 4)}…{r.id.slice(-4)}</span>{/if}
            </span>
          </label>
          {#if r.status === 'different' && selected[r.key]}
            <Segmented
              label="Which version of {r.label}"
              value={theirs[r.key] ? 'theirs' : 'mine'}
              options={[
                { value: 'mine', label: 'Mine' },
                { value: 'theirs', label: 'Theirs' },
              ]}
              onchange={(v) => (theirs[r.key] = v === 'theirs')}
            />
          {:else}
            <span class="tag {r.status}">{STATUS[r.status]}</span>
          {/if}
        </li>
      {/each}
    </ul>

    {#if preview.has_settings}
      <div class="opt">
        <div><div>Sound, overlay and general settings</div><div class="sub">From the file, merged into yours.</div></div>
        <Switch label="Also import settings" checked={importSettings} onchange={(v) => (importSettings = v)} />
      </div>
    {/if}
    {#if preview.has_credentials}
      <div class="opt">
        <div>
          <div>Discord app details</div>
          <div class="sub">Saved in your system keychain.{source.migration ? '' : ' Only import these from your own files.'}</div>
        </div>
        <Switch label="Also import Discord app details" checked={importCreds} onchange={(v) => (importCreds = v)} />
      </div>
    {/if}
    {#if source.migration && (preview.has_token || preview.has_avatars)}
      <p class="note">
        {preview.has_token ? 'Your saved Discord approval comes along, so Discord won’t ask again. ' : ''}{preview.has_avatars ? 'Saved profile pictures come along too.' : ''}
      </p>
    {/if}
    {#if preview.missing_sounds.length}
      <p class="note">
        {preview.missing_sounds.length === 1 ? `1 sound file isn’t in this file, so ${preview.missing_sounds[0]} uses` : `${preview.missing_sounds.length} sound files aren’t in this file, so those people use`} your universal sound.
      </p>
    {/if}
    {#if preview.skipped.length}
      <Button kind="link" small onclick={() => (showSkipped = !showSkipped)}>
        {showSkipped ? 'Hide' : 'Show'} {preview.skipped.length} skipped {preview.skipped.length === 1 ? 'entry' : 'entries'}
      </Button>
      {#if showSkipped}
        <ul class="skipped" in:unfold out:fold>
          {#each preview.skipped as s}
            <li><span class="id">{s.value}</span>: {s.reason}</li>
          {/each}
        </ul>
      {/if}
    {/if}
    {#if error}{#key error}<p class="error" role="alert" in:swapIn>{error}</p>{/key}{/if}
  {/if}

  {#snippet footer()}
    <Button kind="ghost" onclick={close}>Cancel</Button>
    <Button kind="primary" disabled={!preview || busy || (count === 0 && !importSettings && !importCreds)} swap={String(count)} onclick={apply}>
      {count === 1 ? 'Import 1 person' : `Import ${count} people`}
    </Button>
  {/snippet}
</Sheet>

<style>
  .muted {
    color: var(--text-3);
  }
  .error {
    margin: var(--sp-2) 0 0;
    color: var(--danger-text);
  }
  .file {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-md);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--accent-text);
  }
  .file-text {
    min-width: 0;
  }
  .file-name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--text);
    font-weight: var(--fw-medium);
  }
  .file-kind {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .counts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-2);
    margin: var(--sp-3) 0;
  }
  .counts div {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-md);
    background: var(--bg);
  }
  dt {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  dd {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: var(--fw-bold);
  }
  dd.new {
    color: var(--live-text);
  }
  dd.diff {
    color: var(--warn);
  }
  .row {
    margin-bottom: var(--sp-2);
  }
  .rows {
    max-height: 230px;
    margin: var(--sp-2) 0;
    padding: var(--sp-1);
    overflow: auto;
    list-style: none;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .rows li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--h-lg);
    padding: 0 var(--sp-2);
    border-radius: var(--r-sm);
  }
  .rows li:hover {
    background: var(--hover);
  }
  .dim .name {
    color: var(--text-3);
  }
  .pick {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: var(--sp-2);
  }
  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }
  .who {
    display: flex;
    min-width: 0;
    align-items: baseline;
    gap: var(--sp-2);
    overflow: hidden;
    white-space: nowrap;
  }
  .who .id {
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag {
    flex: none;
    font-size: var(--fs-xs);
    font-weight: var(--fw-medium);
    color: var(--text-3);
  }
  .tag.new {
    color: var(--live-text);
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-top: 1px solid var(--line);
  }
  .sub {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .note {
    margin: var(--sp-2) 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .note.warn {
    color: var(--warn);
  }
  .skipped {
    margin: var(--sp-1) 0 0;
    padding-left: var(--sp-5);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
</style>
