<script lang="ts">
  // Share a list with friends, or back everything up.
  import Sheet from '../components/Sheet.svelte';
  import Button from '../components/Button.svelte';
  import Icon from '../components/Icon.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Switch from '../components/Switch.svelte';
  import TextField from '../components/TextField.svelte';
  import { app } from '../lib/app.svelte';
  import { call, errText, saveFile } from '../lib/api';
  import { toast } from '../lib/app.svelte';
  import { fold, iconIn, iconOut, unfold } from '../lib/motion';
  import { keyOf, sortTargets } from '../lib/util';
  import type { ExportOptions } from '../lib/types';

  let { onclose }: { onclose: () => void } = $props();
  const cfg = $derived(app.snap!.config);

  let kind = $state<ExportOptions['kind']>('watchlist');
  let who = $state<'all' | 'some'>('all');
  let picked = $state<Record<string, boolean>>({});
  let name = $state('');
  let embed = $state(true);
  let creds = $state(false);
  let copied = $state(false);
  let status = $state('');

  const options = $derived<ExportOptions>({
    kind,
    keys: who === 'some' ? Object.keys(picked).filter((k) => picked[k]) : null,
    embed_sounds: embed,
    include_credentials: kind === 'backup' && creds,
    name: kind === 'watchlist' && name.trim() ? name.trim() : null,
  });
  const count = $derived(who === 'some' ? Object.values(picked).filter(Boolean).length : cfg.targets.length);

  async function copy() {
    try {
      const text = await call<string>('export_text', { options });
      await navigator.clipboard.writeText(text);
      copied = true;
      status = 'Copied to the clipboard';
      setTimeout(() => ((copied = false), (status = '')), 2000);
    } catch (e) {
      toast('error', errText(e));
    }
  }

  async function save() {
    try {
      const file = await call<string>('export_filename', { options });
      const path = await saveFile('Save the list', file);
      if (!path) return;
      await call('export_file', { options, path });
      toast('ok', 'Saved');
      onclose();
    } catch (e) {
      toast('error', errText(e));
    }
  }

  const ABOUT = {
    watchlist: 'Names, usernames and per-person beeps. Never your Discord app details or settings.',
    backup: 'Everything: your list, sounds and settings. Keep it for yourself.',
  };
</script>

<Sheet title="Export" width={460} {onclose}>
  <div class="grid">
    <Segmented
      label="What to export"
      value={kind}
      options={[
        { value: 'watchlist', label: 'List for friends' },
        { value: 'backup', label: 'Full backup' },
      ]}
      onchange={(v) => (kind = v)}
    />
    <p class="about">{ABOUT[kind]}</p>

    {#if kind === 'watchlist'}
      <div in:unfold out:fold>
        <span class="lbl">List name (optional)</span>
        <TextField label="List name" placeholder="Friday squad" bind:value={name} />
      </div>
    {/if}

    {#if kind !== 'backup'}
      <div class="opt">
        <span>Who</span>
        <Segmented
          label="Who to include"
          value={who}
          options={[
            { value: 'all', label: `Everyone (${cfg.targets.length})` },
            { value: 'some', label: 'Choose people' },
          ]}
          onchange={(v) => (who = v)}
        />
      </div>
      {#if who === 'some'}
        <ul class="pick" in:unfold out:fold aria-label="People to include">
          {#each sortTargets(cfg.targets) as t (keyOf(t))}
            <li>
              <label><input type="checkbox" bind:checked={picked[keyOf(t)]} /> {t.label}</label>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}

    <div class="opt">
      <div><div>Include sound files</div><div class="sub">Custom sound files go inside the .json, so they arrive too.</div></div>
      <Switch label="Include sound files" checked={embed} onchange={(v) => (embed = v)} />
    </div>
    {#if kind === 'backup'}
      <div class="opt">
        <div><div>Include Discord app details</div><div class="sub">Only for moving to another computer of yours.</div></div>
        <Switch label="Include Discord app details" checked={creds} onchange={(v) => (creds = v)} />
      </div>
      {#if creds}
        <p class="warn" in:unfold out:fold>
          <Icon name="warning" size={16} /> This file will contain your Client Secret. Don't share it.
        </p>
      {/if}
    {/if}
  </div>
  <div class="sr-only" role="status" aria-live="polite">{status}</div>

  {#snippet footer()}
    <Button kind="ghost" disabled={count === 0} onclick={copy} label={copied ? 'Copied' : 'Copy to clipboard'}>
      <span class="ic">
        {#key copied}<span class="ic-in" in:iconIn out:iconOut><Icon name={copied ? 'check' : 'content_copy'} size={16} /></span>{/key}
      </span>
      {copied ? 'Copied' : 'Copy'}
    </Button>
    <Button kind="primary" disabled={count === 0} onclick={save}>Save file…</Button>
  {/snippet}
</Sheet>

<style>
  .grid {
    display: grid;
    gap: var(--sp-3);
  }
  .about {
    margin: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .lbl {
    display: block;
    margin-bottom: var(--sp-1);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .sub {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .pick {
    max-height: 180px;
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    overflow: auto;
    list-style: none;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .pick label {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--h-md);
  }
  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: 0;
    color: var(--danger-text);
    font-size: var(--fs-sm);
  }
  .ic {
    position: relative;
    display: inline-flex;
    width: 16px;
    height: 16px;
  }
  .ic-in {
    display: inline-flex;
  }
</style>
