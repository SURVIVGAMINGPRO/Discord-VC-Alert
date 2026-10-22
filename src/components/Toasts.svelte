<script lang="ts">
  // The notification stack (Animate UI's toast, as CSS): the newest toast at
  // the front, up to 3 behind it scaled down and peeking out above. Hovering
  // or focusing the stack opens it, front to back, and pauses every timer.
  // A toast rises its own height from the bottom on the toast spring, growing
  // out of a light blur, and leaves in place on the dismiss curve.
  import Toast from './Toast.svelte';
  import { toasts } from '../lib/app.svelte';
  import { ms, num } from '../lib/motion';

  let { bottom = 16 }: { bottom?: number } = $props();

  let heights = $state<Record<number, number>>({});
  let hovered = $state(false);
  let focused = $state(false);
  let hidden = $state(typeof document !== 'undefined' && document.visibilityState === 'hidden');
  const open = $derived(hovered || focused);

  // The toasts that aren't leaving, newest first.
  const live = $derived(toasts.list.filter((t) => !t.leaving));

  type Pos = { y: number; s: number; h: number; i: number };
  const pos = $derived.by(() => {
    const gap = num('--toast-gap');
    const peek = num('--toast-peek');
    const front = heights[live[0]?.id] ?? 0;
    const out: Record<number, Pos> = {};
    let above = 0;
    live.forEach((t, i) => {
      const own = heights[t.id] ?? front;
      if (open) {
        out[t.id] = { y: -above - i * gap, s: 1, h: own, i };
        above += own;
      } else {
        // Behind: as tall as the front one, scaled from the bottom, its top
        // edge `peek` above the toast in front of it.
        const s = Math.max(0.9, 1 - i * 0.05);
        out[t.id] = { y: i ? -(1 - s) * front - i * peek : 0, s, h: i ? front : own, i };
      }
    });
    return out;
  });

  // A leaving toast stays where it was while it fades.
  const last = new Map<number, Pos>();
  $effect(() => {
    for (const [id, p] of Object.entries(pos)) last.set(Number(id), p);
  });
  const at = (id: number): Pos => pos[id] ?? last.get(id) ?? { y: 0, s: 1, h: heights[id] ?? 0, i: 0 };

  // The stagger applies only while the stack opens or closes, so a toast
  // arriving later never waits for it.
  let stagger = $state(false);
  let staggerTimer: ReturnType<typeof setTimeout> | undefined;
  let wasOpen = false;
  $effect(() => {
    if (open === wasOpen) return;
    wasOpen = open;
    stagger = true;
    clearTimeout(staggerTimer);
    staggerTimer = setTimeout(() => (stagger = false), ms('--spring-toast-dur') + 3 * ms('--stagger-stack'));
  });

  // Nothing left under the pointer: the stack is closed again.
  $effect(() => {
    if (!toasts.list.length) hovered = focused = false;
  });

  // Start each toast below its rest, then let it rise (a style change after
  // the first one is computed, so the transition runs).
  function rise(node: HTMLElement) {
    node.dataset.phase = 'enter';
    getComputedStyle(node).transform;
    node.dataset.phase = 'live';
  }
</script>

<svelte:document onvisibilitychange={() => (hidden = document.visibilityState === 'hidden')} />

<div
  class="region"
  class:stagger
  style:bottom="{bottom}px"
  role="status"
  aria-live="polite"
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => (hovered = false)}
  onfocusin={() => (focused = true)}
  onfocusout={(e) => {
    const next = e.relatedTarget as Node | null;
    if (!next || !e.currentTarget.contains(next)) focused = false;
  }}
>
  {#each toasts.list as t (t.id)}
    {@const p = at(t.id)}
    <div
      class="toast"
      class:leaving={t.leaving}
      inert={t.leaving}
      use:rise
      style:--y={p.y}
      style:--s={p.s}
      style:--h={p.h ? `${p.h}px` : 'auto'}
      style:--i={p.i}
    >
      <div class="clip">
        <Toast {t} paused={open || hidden} behind={!open && p.i > 0} onmeasure={(h) => (heights[t.id] = h)} />
      </div>
    </div>
  {/each}
</div>

<style>
  .region {
    position: fixed;
    left: var(--sp-4);
    right: var(--sp-4);
    z-index: 70;
    height: 0;
    pointer-events: none;
  }
  /* Wide windows (Settings): bottom right, at most 384 px wide. */
  @media (min-width: 640px) {
    .region {
      left: auto;
      width: 384px;
    }
  }
  .toast {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 100%;
    height: var(--h);
    z-index: calc(10 - var(--i));
    border-radius: var(--r-lg);
    background: var(--raised);
    box-shadow:
      var(--shadow-pop),
      inset 0 0 0 1px var(--line);
    transform: translateY(calc(var(--y) * 1px + var(--enter, 0%))) scale(calc(var(--s) * var(--grow, 1)));
    transform-origin: bottom center;
    pointer-events: auto;
    transition:
      transform var(--spring-toast-dur) var(--spring-toast-ease) var(--delay, 0ms),
      height var(--spring-toast-dur) var(--spring-toast-ease) var(--delay, 0ms),
      opacity var(--t-fade) var(--ease-out),
      filter var(--t-fade) var(--ease-out);
  }
  /* Fills the gap below each toast, so moving between them keeps it open. */
  .toast::after {
    content: '';
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    height: calc(var(--toast-gap) + 1px);
  }
  .clip {
    position: relative;
    height: 100%;
    overflow: hidden;
    border-radius: inherit;
  }
  .stagger .toast {
    --delay: calc(var(--i) * var(--stagger-stack));
  }
  .toast:global([data-phase='enter']) {
    --enter: 100%;
    --grow: var(--toast-from);
    opacity: 0;
    filter: blur(var(--blur-surface));
  }
  .toast.leaving {
    --grow: var(--toast-from);
    opacity: 0;
    filter: blur(var(--blur-surface));
    pointer-events: none;
    transition:
      transform var(--t-dismiss) var(--ease-away),
      opacity var(--t-dismiss) var(--ease-away),
      filter var(--t-dismiss) var(--ease-away);
  }
  /* Reduced motion: toasts fade in and out of their blur; nothing rises,
     grows or slides, and the stack opens at once. */
  @media (prefers-reduced-motion: reduce) {
    .toast,
    .toast.leaving {
      transition:
        opacity var(--t-fade) var(--ease-out),
        filter var(--t-fade) var(--ease-out);
    }
    .toast:global([data-phase='enter']),
    .toast.leaving {
      --enter: 0%;
      --grow: 1;
    }
  }
</style>
