<script lang="ts">
  // One person on the watch list, as a card (layout from the early mockup).
  // The line under the name shows who they are (@username or ID, unless the
  // label already is the username) and, while they're in your channel, how
  // long ago they joined. Right side: "Snoozed · 52 min" while snoozed, else
  // "Custom" (or "Custom · Urgent") for their own sound, and a round More
  // button (Rename, Sound…, Snooze, Copy, Remove) that takes its place on hover
  // or focus.
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
    tag,
    snooze,
    onmenu,
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
    tag: string | null;
    snooze: string | null;
    onmenu: (x: number, y: number) => void;
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
  const shownTag = $derived(snooze ?? tag);

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
  aria-label="{t.label}{ident ? `, ${ident}` : ''}{present ? ', in your channel' : ''}{ownSound ? ', has its own sound' : ''}{snooze ? ', snoozed' : ''}"
  tabindex={active ? 0 : -1}
  onfocus={onfocusrow}
  onkeydown={key_}
  oncontextmenu={(e) => {
    e.preventDefault();
    onmenu(e.clientX, e.clientY);
  }}
>
  <Avatar {key} label={t.label} size="var(--avatar)" {src} {present} hint="Their picture appears after their first call with you" />
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
    {#if ident || since}
      <div class="sub">
        {#if ident}<span class="ident id">{ident}</span>{/if}
        {#if ident && since}<span class="sep" aria-hidden="true">·</span>{/if}
        {#if since}<span class="since">{since}</span>{/if}
      </div>
    {/if}
  </div>
  <div class="side">
    {#if shownTag}<span class="tag" class:quiet={!!snooze} title={snooze ? 'No join or leave sound for them' : 'Has their own sound'}>{shownTag}</span>{/if}
    <span class="acts" bind:this={more}>
      <IconButton round flat={false} size={28} icon="more_horiz" label="More for {t.label}" tabindex={-1} onclick={openFromButton} />
    </span>
  </div>
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: var(--row-h);
    /* The picture is centred in the card's height and has the same space to
       its left as above and below it. */
    padding: var(--row-y) var(--sp-3) var(--row-y) calc((var(--row-h) - var(--avatar)) / 2);
    border-radius: var(--r-md);
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1px var(--line),
      inset 0 0 0 0 transparent;
    outline: none;
    --dot-ring: var(--surface);
    transition:
      background-color var(--t-fade) var(--ease-color),
      box-shadow var(--t-inset) var(--ease-inset);
  }
  /* Hover sinks the card a little: a soft shadow grows in from its edges. */
  .row:hover {
    box-shadow:
      inset 0 0 0 1px var(--line),
      inset 0 0 14px 0 var(--card-inset);
  }
  .row:hover,
  .row:focus-within {
    background: var(--hover);
    --dot-ring: var(--hover);
  }
  .row:focus-visible {
    box-shadow: inset 0 0 0 2px var(--focus);
    /* Focus rings appear at once, never through the hover shadow's fade. */
    transition: background-color var(--t-fade) var(--ease-color);
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
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  .sub {
    display: flex;
    align-items: baseline;
    gap: var(--sp-1);
    min-width: 0;
    margin-top: var(--sp-0);
    white-space: nowrap;
    font-size: var(--fs-sm);
    line-height: var(--lh-tight);
    /* Two fonts on one line (mono username, sans "joined…") mustn't make it taller. */
    height: calc(var(--fs-sm) * var(--lh-tight));
  }
  .ident {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-3);
  }
  .sep {
    flex: none;
    color: var(--text-3);
  }
  .since {
    flex: 0 1000 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--live-text);
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
  /* The tag and the buttons share one spot, so nothing shifts on hover. */
  .side {
    display: grid;
    flex: none;
    align-items: center;
    justify-items: end;
  }
  .side > * {
    grid-area: 1 / 1;
    transition: opacity var(--t-fade) var(--ease-color);
  }
  .tag {
    padding: var(--sp-0) var(--sp-2);
    border-radius: var(--r-full);
    background: var(--accent-wash);
    color: var(--accent-text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
    line-height: var(--lh-tight);
  }
  .tag.quiet {
    background: var(--hover);
    color: var(--text-2);
  }
  .acts {
    display: flex;
    opacity: 0;
  }
  .row:hover .acts,
  .row:focus-within .acts {
    opacity: 1;
  }
  /* The tag steps aside at once, so it never shows through the buttons. */
  .row:hover .tag,
  .row:focus-within .tag {
    opacity: 0;
    transition-duration: 0s;
  }
</style>
