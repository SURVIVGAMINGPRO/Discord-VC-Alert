<script lang="ts">
  // One person on the watch list. The line under the name always shows who
  // they are (@username or ID), unless the label already is the username.
  import Avatar from '../components/Avatar.svelte';
  import IconButton from '../components/IconButton.svelte';
  import type { Target } from '../lib/types';
  import { keyOf, mask } from '../lib/util';

  let {
    t,
    present,
    since,
    masked,
    src,
    joined,
    active,
    renaming,
    onmenu,
    onsound,
    onremove,
    onrename,
    onfocusrow,
    onmove,
  }: {
    t: Target;
    present: boolean;
    since: string | null;
    masked: boolean;
    src: string | null;
    joined: boolean;
    active: boolean;
    renaming: boolean;
    onmenu: (x: number, y: number) => void;
    onsound: () => void;
    onremove: () => void;
    onrename: (label: string | null) => void;
    onfocusrow: () => void;
    onmove: (dir: 1 | -1 | 'first' | 'last') => void;
  } = $props();

  const key = $derived(keyOf(t));
  const sameAsUser = $derived(!!t.username && t.username.toLowerCase() === t.label.trim().toLowerCase());
  const name = $derived(masked && sameAsUser ? mask(t.label) : t.label);
  const ident = $derived.by(() => {
    if (t.id) {
      const id = masked ? mask(t.id) : t.id;
      return `ID ${id.slice(0, 4)}…${id.slice(-4)}`;
    }
    if (sameAsUser) return null;
    return '@' + (masked ? mask(t.username ?? '') : (t.username ?? ''));
  });
  const ownSound = $derived(!!t.sound);

  let row: HTMLDivElement | undefined = $state();
  let more: HTMLElement | undefined = $state();
  let draft = $state('');
  let field: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (renaming) {
      draft = t.label;
      queueMicrotask(() => field?.select());
    }
  });

  export function focus() {
    row?.focus();
  }

  function openFromButton() {
    const r = (more ?? row)!.getBoundingClientRect();
    onmenu(r.right - 4, r.bottom + 4);
  }

  function key_(e: KeyboardEvent) {
    if (renaming || e.target !== row) return;
    switch (e.key) {
      case 'ArrowDown':
        onmove(1);
        break;
      case 'ArrowUp':
        onmove(-1);
        break;
      case 'Home':
        onmove('first');
        break;
      case 'End':
        onmove('last');
        break;
      case 'Enter':
      case ' ':
      case 'ContextMenu':
        openFromButton();
        break;
      case 'F10':
        if (!e.shiftKey) return;
        openFromButton();
        break;
      case 'Delete':
      case 'Backspace':
        onremove();
        break;
      case 'F2':
        onrename(null);
        break;
      default:
        return;
    }
    e.preventDefault();
  }
</script>

<!-- A focusable list item with a roving tabindex: arrows move between people,
     Enter opens the actions menu, Delete removes. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  bind:this={row}
  class="row"
  class:joined
  class:present
  class:own={ownSound}
  role="listitem"
  aria-label="{t.label}{ident ? `, ${ident}` : ''}{present ? ', in your channel' : ''}{ownSound ? ', has its own sound' : ''}"
  tabindex={active ? 0 : -1}
  onfocus={onfocusrow}
  onkeydown={key_}
  oncontextmenu={(e) => {
    e.preventDefault();
    onmenu(e.clientX, e.clientY);
  }}
>
  <Avatar {key} label={t.label} size={30} {src} {present} />
  <div class="who">
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="rename"
        bind:this={field}
        bind:value={draft}
        aria-label="Name for {key}"
        autofocus
        onkeydown={(e) => {
          if (e.key === 'Enter') onrename(draft);
          if (e.key === 'Escape') onrename(t.label);
          e.stopPropagation();
        }}
        onblur={() => onrename(draft)}
      />
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="name" title={t.label} ondblclick={() => onrename(null)}>{name}</div>
    {/if}
    {#if ident}<div class="ident id">{ident}</div>{/if}
  </div>
  {#if since}<span class="since num" title="Time since they joined">{since}</span>{/if}
  <div class="acts">
    <span class="sound">
      <IconButton
        icon="volume_up"
        label={ownSound ? `${t.label} has their own sound` : `Choose a sound for ${t.label}`}
        tone={ownSound ? 'accent' : undefined}
        tabindex={-1}
        onclick={onsound}
      />
    </span>
    <span class="more" bind:this={more}>
      <IconButton icon="more_horiz" label="More for {t.label}" tabindex={-1} onclick={openFromButton} />
    </span>
  </div>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: calc(var(--avatar) + 2 * var(--row-y));
    padding: var(--row-y) var(--sp-2);
    border-radius: var(--r-md);
    outline: none;
    transition: background-color var(--t-fade) var(--ease-color);
  }
  .row:hover,
  .row:focus-within {
    background: var(--hover);
    --dot-ring: var(--hover);
  }
  .row:focus-visible {
    box-shadow: inset 0 0 0 2px var(--focus);
  }
  /* The one big moment in the window: a soft green wash that fades out. */
  .row::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--live-wash);
    opacity: 0;
    pointer-events: none;
  }
  .joined::after {
    animation: joined var(--t-ping) var(--ease-out);
  }
  @keyframes joined {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-weight: var(--fw-medium);
  }
  .ident {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--text-3);
  }
  .rename {
    width: 100%;
    height: var(--h-sm);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-wash);
    color: var(--text);
    font-weight: var(--fw-medium);
    outline: none;
  }
  .since {
    flex: none;
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .acts {
    display: flex;
    flex: none;
    gap: var(--sp-0);
  }
  .sound,
  .more {
    display: flex;
    opacity: 0;
    transition: opacity var(--t-fade) var(--ease-color);
  }
  .own .sound,
  .row:hover .sound,
  .row:hover .more,
  .row:focus-within .sound,
  .row:focus-within .more {
    opacity: 1;
  }
</style>
