<script lang="ts">
  // "Position the overlay": size, layout, snap to a corner, Save or Cancel.
  import { onMount } from 'svelte';
  import Button from '../components/Button.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Slider from '../components/Slider.svelte';
  import { call, on } from '../lib/api';
  import type { EditorModel } from '../lib/types';

  let m = $state<EditorModel | null>(null);
  onMount(() => {
    const off = on<EditorModel>('editor', (x) => (m = x));
    call('editor_refresh');
    return () => off.then((f) => f());
  });

  const CORNERS = [
    { id: 'top-left', label: 'Top left' },
    { id: 'top-right', label: 'Top right' },
    { id: 'bottom-left', label: 'Bottom left' },
    { id: 'bottom-right', label: 'Bottom right' },
  ];

  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape') call('editor_cancel');
    if (e.key === 'Enter' && !(e.target instanceof HTMLButtonElement)) call('editor_save');
  }
</script>

<svelte:window onkeydown={keys} />

<div class="panel">
  <h1>Position the overlay</h1>
  <p class="lead">Drag the outlined bar anywhere on your screen. Pictures line up from the nearest corner. Nothing changes until you save.</p>
  {#if m}
    <div class="grid">
      <span class="lbl">Size</span>
      <Slider label="Picture size" value={m.px} min={8} max={48} step={2} format={(v) => `${v} px`} onchange={(px) => call('editor_update', { px, layout: m!.layout })} />
      <span class="lbl">Layout</span>
      <Segmented
        label="Layout"
        value={m.layout}
        options={[
          { value: 'row', label: 'Row' },
          { value: 'column', label: 'Column' },
          { value: 'grid', label: 'Grid' },
        ]}
        onchange={(layout) => call('editor_update', { px: m!.px, layout })}
      />
      <span class="lbl">Snap to</span>
      <div class="snap">
        {#each CORNERS as c}
          <Button small onclick={() => call('editor_snap', { corner: c.id })}>{c.label}</Button>
        {/each}
      </div>
    </div>
    <p class="readout num" aria-live="polite">{m.readout}</p>
  {/if}
  <div class="actions">
    <Button kind="ghost" onclick={() => call('editor_cancel')}>Cancel</Button>
    <Button kind="primary" onclick={() => call('editor_save')}>Save position</Button>
  </div>
</div>

<style>
  .panel {
    display: grid;
    gap: var(--sp-3);
    padding: var(--sp-5);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
  }
  .lead {
    margin: 0;
    color: var(--text-2);
  }
  .grid {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    padding: var(--sp-3);
    border-radius: var(--r-md);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .lbl {
    color: var(--text-2);
  }
  .snap {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-1);
  }
  .readout {
    margin: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
  }
</style>
