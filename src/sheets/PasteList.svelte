<script lang="ts">
  // Paste usernames (added straight away) or a JSON list (goes to the import preview).
  import Sheet from '../components/Sheet.svelte';
  import Button from '../components/Button.svelte';
  import { call, errText } from '../lib/api';
  import { toast } from '../lib/app.svelte';
  import { shake } from '../lib/motion';
  import { keyOf } from '../lib/util';
  import type { Target } from '../lib/types';

  let { onclose, onjson }: { onclose: () => void; onjson: (text: string) => void } = $props();
  let text = $state('');
  let error = $state('');
  let area: HTMLTextAreaElement | undefined = $state();
  const isJson = $derived(/^\s*[[{]/.test(text));

  async function go() {
    if (!text.trim()) {
      error = 'Paste some usernames or a VC Alert list first.';
      shake(area);
      return;
    }
    if (isJson) return onjson(text);
    try {
      const r = await call<{ added: Target[]; skipped: { value: string; reason: string }[] }>('add_many', { text });
      if (!r.added.length) {
        error = r.skipped.length ? `Nothing new to add. ${r.skipped.length} skipped (${r.skipped[0].reason}).` : 'Nothing to add.';
        shake(area);
        return;
      }
      const keys = r.added.map(keyOf);
      toast('ok', `Added ${keys.length === 1 ? '1 person' : `${keys.length} people`}${r.skipped.length ? `, skipped ${r.skipped.length}` : ''}`, {
        action: { label: 'Undo', run: () => keys.forEach((key) => call('remove_target', { key })) },
      });
      onclose();
    } catch (e) {
      error = errText(e);
    }
  }
</script>

<Sheet title="Paste a list" subtitle="Usernames or IDs separated by commas, spaces or new lines. A VC Alert .json works too." width={460} {onclose}>
  <textarea
    bind:this={area}
    bind:value={text}
    class:invalid={!!error}
    aria-label="Usernames or a VC Alert list"
    aria-describedby="paste-error"
    placeholder="moderator, maria_k, 123456789012345678"
    spellcheck="false"
    oninput={() => (error = '')}
  ></textarea>
  <p class="msg" class:error={!!error} id="paste-error" aria-live="polite">
    {error || (isJson ? 'This looks like a VC Alert file. You’ll see what’s in it before anything changes.' : ' ')}
  </p>
  {#snippet footer()}
    <Button kind="ghost" onclick={onclose}>Cancel</Button>
    <Button kind="primary" swap={isJson ? 'json' : 'names'} onclick={go}>{isJson ? 'Preview import' : 'Add people'}</Button>
  {/snippet}
</Sheet>

<style>
  textarea {
    display: block;
    width: 100%;
    min-height: 140px;
    padding: var(--sp-2) var(--sp-3);
    border: 0;
    border-radius: var(--r-sm);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--text);
    font-family: var(--font-id);
    font-size: var(--fs-sm);
    resize: vertical;
    outline: none;
  }
  textarea::placeholder {
    color: var(--text-3);
  }
  textarea:focus {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-wash);
  }
  textarea.invalid {
    box-shadow:
      inset 0 0 0 1px var(--danger),
      0 0 0 3px var(--danger-wash);
  }
  .msg {
    min-height: 1.4em;
    margin: var(--sp-2) 0 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .msg.error {
    color: var(--danger-text);
  }
</style>
