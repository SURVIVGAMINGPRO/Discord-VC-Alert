<script lang="ts" generics="T extends string">
  // One highlight slides to the selected option. Arrow keys move the choice.
  import { slideHighlight } from '../lib/motion';
  let {
    options,
    value,
    label,
    onchange,
  }: { options: { value: T; label: string }[]; value: T; label: string; onchange: (v: T) => void } = $props();

  let box: HTMLDivElement | undefined = $state();
  let hl: HTMLSpanElement | undefined = $state();
  let mounted = false;

  function place(animate: boolean) {
    const target = box?.querySelector<HTMLElement>('[aria-checked="true"]');
    slideHighlight(hl, target, animate);
  }

  $effect(() => {
    void value;
    void options.length;
    place(mounted);
    mounted = true;
  });

  $effect(() => {
    if (!box) return;
    const ro = new ResizeObserver(() => place(false));
    ro.observe(box);
    return () => ro.disconnect();
  });

  function key(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const next = options[(i + d + options.length) % options.length];
    onchange(next.value);
    queueMicrotask(() => box?.querySelector<HTMLElement>(`[data-v="${next.value}"]`)?.focus());
  }
</script>

<div class="seg" role="radiogroup" aria-label={label} bind:this={box} onkeydown={key} tabindex="-1">
  <span class="hl" bind:this={hl} aria-hidden="true"></span>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      data-v={o.value}
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      class:sel={o.value === value}
      onclick={() => o.value !== value && onchange(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--sp-0);
    padding: var(--sp-0);
    border-radius: var(--r-sm);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .seg:focus {
    outline: none;
  }
  .hl {
    position: absolute;
    z-index: 0;
    opacity: 0;
    border-radius: calc(var(--r-sm) - 2px);
    background: var(--raised);
    box-shadow:
      inset 0 0 0 1px var(--line-strong),
      var(--shadow-pop);
    transform-origin: 0 0;
    pointer-events: none;
  }
  button {
    position: relative;
    z-index: 1;
    height: var(--h-sm);
    padding: 0 var(--sp-3);
    border: 0;
    border-radius: calc(var(--r-sm) - 2px);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
    white-space: nowrap;
    transition:
      color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  button:hover:not(.sel) {
    color: var(--text);
  }
  button:active {
    transform: scale(var(--press-scale));
  }
  .sel {
    color: var(--text);
  }
  @media (prefers-reduced-motion: reduce) {
    button:active {
      transform: none;
    }
  }
</style>
