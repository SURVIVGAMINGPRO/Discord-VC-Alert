<script lang="ts">
  import Sheet from '../components/Sheet.svelte';
  import TextField from '../components/TextField.svelte';
  import Button from '../components/Button.svelte';
  import { call, errText } from '../lib/api';
  import { toast } from '../lib/app.svelte';
  import { shake } from '../lib/motion';
  import { keyOf, parseIdent, PARSE_ERROR } from '../lib/util';
  import type { Target } from '../lib/types';

  let { onclose, onadded }: { onclose: () => void; onadded: (t: Target) => void } = $props();
  let label = $state('');
  let ident = $state('');
  let error = $state('');
  let box: HTMLDivElement | undefined = $state();

  async function add() {
    if (!ident.trim()) {
      error = 'Enter a username or user ID.';
      shake(box);
      return;
    }
    if (!parseIdent(ident)) {
      error = PARSE_ERROR;
      shake(box);
      return;
    }
    try {
      const t = await call<Target>('add_target', { label, ident });
      const k = keyOf(t);
      toast('ok', `Added ${t.label}`, { action: { label: 'Undo', run: () => call('remove_target', { key: k }) } });
      onadded(t);
    } catch (e) {
      error = errText(e);
      shake(box);
    }
  }
</script>

<Sheet title="Add someone to watch" subtitle="Use their unique Discord username, not their display name." width={420} {onclose}>
  <div class="form">
    <span class="lbl">Name in your list</span>
    <TextField label="Name in your list" placeholder="Moderator" bind:value={label} onenter={add} />
    <span class="lbl">Username or user ID</span>
    <div bind:this={box}>
      <TextField
        label="Username or user ID"
        placeholder="moderator"
        mono
        invalid={!!error}
        describedby="add-error"
        bind:value={ident}
        oninput={() => (error = '')}
        onenter={add}
      />
    </div>
    <p class="msg" class:error={!!error} id="add-error" aria-live="polite">
      {error || 'The name is optional. Leave it empty to show the username.'}
    </p>
  </div>
  {#snippet footer()}
    <Button kind="ghost" onclick={onclose}>Cancel</Button>
    <Button kind="primary" onclick={add}>Add</Button>
  {/snippet}
</Sheet>

<style>
  .form {
    display: grid;
    gap: var(--sp-1);
  }
  .lbl {
    margin-top: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  .lbl:first-child {
    margin-top: 0;
  }
  .msg {
    margin: var(--sp-1) 0 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .msg.error {
    color: var(--danger-text);
  }
</style>
