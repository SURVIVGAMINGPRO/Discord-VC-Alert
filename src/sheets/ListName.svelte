<script lang="ts">
  // Name a new list, or rename one: up to 12 characters, unique ignoring case
  // (the engine has the final say). A new list becomes the watched one.
  import Sheet from '../components/Sheet.svelte';
  import TextField from '../components/TextField.svelte';
  import Button from '../components/Button.svelte';
  import { toast } from '../lib/app.svelte';
  import { call, errText } from '../lib/api';
  import { shake } from '../lib/motion';
  import { LIST_NAME_MAX } from '../lib/util';
  import { untrack } from 'svelte';

  let { index = null, current = '', onclose }: { index?: number | null; current?: string; onclose: () => void } = $props();

  let name = $state(untrack(() => current));
  let error = $state('');
  let box: HTMLDivElement | undefined = $state();
  let field: HTMLInputElement | undefined = $state();
  $effect(() => {
    field?.select();
  });

  async function save() {
    const n = [...name.trim()].slice(0, LIST_NAME_MAX).join('').trim();
    if (!n) {
      error = 'Give the list a name.';
      shake(box);
      return;
    }
    try {
      if (index === null) {
        await call('list_add', { name: n });
        toast('ok', `Created list “${n}”`);
      } else {
        await call('list_rename', { index, name: n });
        toast('ok', `Renamed list to “${n}”`);
      }
      onclose();
    } catch (e) {
      error = errText(e);
      shake(box);
    }
  }
</script>

<Sheet
  title={index === null ? 'New list' : 'Rename list'}
  subtitle={index === null ? 'Each list has its own people and their own sounds.' : undefined}
  width={400}
  {onclose}
>
  <div bind:this={box}>
    <TextField
      label="List name"
      placeholder="Gaming"
      maxlength={LIST_NAME_MAX}
      bind:value={name}
      bind:input={field}
      invalid={!!error}
      describedby="list-name-msg"
      oninput={() => (error = '')}
      onenter={save}
      onescape={onclose}
    />
  </div>
  <p class="msg" class:error={!!error} id="list-name-msg" aria-live="polite">
    {error || `${[...name].length}/${LIST_NAME_MAX} characters`}
  </p>
  {#snippet footer()}
    <Button kind="ghost" onclick={onclose}>Cancel</Button>
    <Button kind="primary" onclick={save}>{index === null ? 'Create list' : 'Rename'}</Button>
  {/snippet}
</Sheet>

<style>
  .msg {
    margin: var(--sp-2) 0 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .msg.error {
    color: var(--danger-text);
  }
</style>
