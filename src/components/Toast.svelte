<script lang="ts">
  // One notification: icon, title, an optional muted line, Undo, ×, and a
  // blurple bar counting down to when it leaves. The timer and the bar pause
  // together (the stack is open, or the window is hidden).
  import { untrack } from 'svelte';
  import Icon from './Icon.svelte';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import { hideToast, type Toast } from '../lib/app.svelte';

  let {
    t,
    paused,
    behind,
    onmeasure,
  }: { t: Toast; paused: boolean; behind: boolean; onmeasure: (h: number) => void } = $props();

  const ICON = { ok: 'check_circle', info: 'info', error: 'cancel' } as const;

  let height = $state(0);
  $effect(() => {
    if (height) onmeasure(height);
  });

  let bar: HTMLSpanElement | undefined = $state();
  let countdown: Animation | undefined;
  let left = untrack(() => t.ms);

  $effect(() => {
    if (!bar) return;
    countdown = bar.animate([{ transform: 'scaleX(1)' }, { transform: 'scaleX(0)' }], {
      duration: untrack(() => t.ms),
      easing: 'linear',
      fill: 'forwards',
    });
    countdown.pause();
    return () => countdown?.cancel();
  });

  $effect(() => {
    if (paused || t.leaving) {
      countdown?.pause();
      return;
    }
    const since = performance.now();
    const timer = setTimeout(() => hideToast(t.id), Math.max(0, left));
    countdown?.play();
    return () => {
      clearTimeout(timer);
      left -= performance.now() - since;
    };
  });
</script>

<div class="card" class:behind>
  <div class="content" bind:offsetHeight={height}>
    <span class="ic {t.kind}" aria-hidden="true"><Icon name={ICON[t.kind]} size={16} /></span>
    <div class="text">
      <p class="title">{t.title}</p>
      {#if t.detail}<p class="detail">{t.detail}</p>{/if}
    </div>
    {#if t.action}
      <Button
        small
        onclick={() => {
          const run = t.action!.run;
          hideToast(t.id);
          run();
        }}>{t.action.label}</Button
      >
    {/if}
    <span class="close">
      <IconButton icon="close" size={24} label="Close notification" onclick={() => hideToast(t.id)} />
    </span>
  </div>
  <span class="bar" bind:this={bar} aria-hidden="true"></span>
</div>

<style>
  .card {
    transition: opacity var(--t-fade) var(--ease-out);
  }
  /* A toast behind the front one shows only its edge. */
  .card.behind {
    opacity: 0;
  }
  .content {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-3) var(--sp-3) var(--sp-4);
  }
  .ic {
    display: flex;
    flex: none;
    color: var(--text-2);
  }
  .ic.error {
    color: var(--danger);
  }
  .text {
    display: grid;
    flex: 1;
    min-width: 0;
    gap: var(--sp-0);
  }
  .title,
  .detail {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .title {
    font-weight: var(--fw-medium);
  }
  .detail {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .close {
    display: flex;
    flex: none;
    margin-left: calc(-1 * var(--sp-1));
  }
  .close :global(.ib) {
    color: var(--text-3);
  }
  .close :global(.ib:hover) {
    color: var(--text);
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: var(--accent);
    transform-origin: left center;
    pointer-events: none;
  }
</style>
