<script lang="ts">
  // A flat button. Press scales to --press-scale; hover only changes colour.
  // Give `swap` a value that changes with the label (e.g. "start" → "stop") and
  // the label crossfades through a light blur while the background resizes
  // smoothly (FLIP on a transform, never on width). The element stays the
  // same, so keyboard focus is kept.
  import type { Snippet } from 'svelte';
  import { play, swapIn, swapOut } from '../lib/motion';

  type Kind = 'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'link';
  let {
    kind = 'secondary',
    small = false,
    disabled = false,
    title,
    onclick,
    children,
    full = false,
    swap,
    label,
  }: {
    kind?: Kind;
    small?: boolean;
    disabled?: boolean;
    title?: string;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
    full?: boolean;
    swap?: string;
    label?: string;
  } = $props();

  let el: HTMLButtonElement | undefined = $state();
  let bg: HTMLSpanElement | undefined = $state();
  let before = 0;

  $effect.pre(() => {
    void swap;
    before = bg ? bg.getBoundingClientRect().width : 0;
  });

  $effect(() => {
    void swap;
    if (!el || !bg || !before) return;
    const after = el.offsetWidth;
    if (Math.abs(after - before) > 0.5) {
      play(bg, [{ transform: `scaleX(${before / after})` }, { transform: 'none' }], '--spring-reflow-dur', '--spring-reflow-ease');
    }
  });
</script>

<button
  bind:this={el}
  type="button"
  class="btn {kind}"
  class:small
  class:full
  {disabled}
  {title}
  aria-label={label}
  {onclick}
>
  <span class="bg" bind:this={bg} aria-hidden="true"></span>
  <span class="label">
    {#if swap !== undefined}
      {#key swap}
        <span class="swap" in:swapIn out:swapOut>{@render children?.()}</span>
      {/key}
    {:else}
      {@render children?.()}
    {/if}
  </span>
</button>

<style>
  .btn {
    --btn-bg: var(--raised);
    --btn-bg-hover: var(--hover);
    --btn-fg: var(--text);
    --btn-line: transparent;
    position: relative;
    isolation: isolate;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    height: var(--h-md);
    padding: 0 var(--sp-3);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--btn-fg);
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
    line-height: 1;
    white-space: nowrap;
    transition: transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .bg {
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    background: var(--btn-bg);
    box-shadow: inset 0 0 0 1px var(--btn-line);
    transition: background-color var(--t-fade) var(--ease-color);
  }
  .label {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }
  .swap {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) .bg {
    background: var(--btn-bg-hover);
  }
  .btn:active:not(:disabled) {
    transform: scale(var(--press-scale));
  }
  .small {
    height: var(--h-sm);
    padding: 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  .full {
    width: 100%;
  }
  .primary {
    --btn-bg: var(--accent);
    --btn-bg-hover: var(--accent-hover);
    --btn-fg: var(--on-accent);
  }
  .success {
    --btn-bg: var(--live);
    --btn-bg-hover: var(--live);
    --btn-fg: var(--on-live);
  }
  .secondary {
    --btn-line: var(--line-strong);
  }
  .ghost {
    --btn-bg: transparent;
    --btn-fg: var(--text-2);
  }
  .ghost:hover:not(:disabled) {
    color: var(--text);
  }
  .danger {
    --btn-bg: var(--danger);
    --btn-bg-hover: var(--danger-hover);
    --btn-fg: var(--on-danger);
  }
  .link {
    --btn-bg: transparent;
    --btn-bg-hover: transparent;
    --btn-fg: var(--accent-text);
    height: auto;
    padding: var(--sp-0) var(--sp-1);
  }
  .link:hover:not(:disabled) {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .btn:disabled {
    --btn-fg: var(--text-3);
    cursor: default;
  }
  .btn:disabled .bg {
    opacity: 0.55;
  }
  @media (prefers-reduced-motion: reduce) {
    .btn:active:not(:disabled) {
      transform: none;
    }
  }
</style>
