<script lang="ts">
  // The draggable ghost bar in setup mode. The whole window is a drag region.
  import { onMount } from 'svelte';
  import { call, on } from '../lib/api';
  import { avatars, loadAvatars, watchAvatars } from '../lib/app.svelte';
  import { avatarColor, initialOf } from '../lib/util';
  import type { EditorModel } from '../lib/types';

  let m = $state<EditorModel | null>(null);
  onMount(() => {
    document.documentElement.dataset.view = 'ghost';
    const off = on<EditorModel>('editor', (x) => {
      m = x;
      loadAvatars(x.cells.flatMap((c) => (c.item.kind === 'person' ? [c.item.key] : [])));
    });
    watchAvatars();
    call('editor_refresh');
    return () => off.then((f) => f());
  });
</script>

<div class="ghost" data-tauri-drag-region title="Drag to move the overlay">
  {#if m}
    {#each m.cells as c, i (i)}
      <span class="slot" data-tauri-drag-region style:left="{c.x}px" style:top="{c.y}px" style:width="{m.slot}px" style:height="{m.slot}px">
        {#if c.item.kind === 'person' && avatars[c.item.key]}
          <img class="face" data-tauri-drag-region src={avatars[c.item.key]} alt="" draggable="false" style:width="{m.px}px" style:height="{m.px}px" />
        {:else if c.item.kind === 'person'}
          <span class="face" data-tauri-drag-region style:background={avatarColor(c.item.key)} style:width="{m.px}px" style:height="{m.px}px" style:font-size="{Math.max(8, Math.round(m.px * 0.44))}px">
            {m.px >= 14 ? initialOf(c.item.label) : ''}
          </span>
        {:else}
          <span class="face empty" data-tauri-drag-region style:width="{m.px}px" style:height="{m.px}px"></span>
        {/if}
      </span>
    {/each}
  {/if}
</div>

<style>
  :global(html[data-view='ghost']),
  :global(html[data-view='ghost'] body) {
    background: var(--ov-ring);
  }
  .ghost {
    position: relative;
    width: 100vw;
    height: 100vh;
    box-shadow: inset 0 0 0 2px var(--accent);
    cursor: move;
  }
  .slot {
    position: absolute;
    margin: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .face {
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    color: var(--ov-text);
    font-weight: var(--fw-bold);
    line-height: 1;
    box-shadow: 0 0 0 2px var(--ov-ring);
    object-fit: cover;
  }
  .empty {
    background: none;
    box-shadow: none;
    outline: 1.5px dashed var(--ov-text-2);
    outline-offset: -1.5px;
  }
</style>
