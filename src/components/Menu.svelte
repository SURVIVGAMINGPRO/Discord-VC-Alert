<script lang="ts">
  // A small menu that grows from where it was opened (the button or pointer).
  // One highlight glides behind the item under the pointer or the arrow keys
  // (Animate UI: spring.highlight, our reflow spring), fading in from nothing
  // and out when the pointer leaves; it turns red behind a destructive item.
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  import { gone, menuPop, slideHighlight } from '../lib/motion';
  export interface MenuItem {
    label: string;
    icon?: IconName;
    danger?: boolean;
    /** A choice among several (the lists): a check when chosen, else an empty slot. */
    checked?: boolean;
    run: () => void;
  }
  let { x, y, items, label = 'Actions', onclose }: { x: number; y: number; items: MenuItem[]; label?: string; onclose: () => void } = $props();
  let el: HTMLDivElement | undefined = $state();
  let pos = $state({ left: 0, top: 0, origin: '0 0' });
  let ready = $state(false);
  let hl: HTMLSpanElement | undefined = $state();
  let lit = false;
  let litDanger = $state(false);

  function light(btn: HTMLElement | null) {
    if (!btn || !hl) return;
    litDanger = btn.classList.contains('danger');
    slideHighlight(hl, btn, lit, 'reflow');
    lit = true;
  }
  function unlight() {
    lit = false;
    if (hl) hl.style.opacity = '0';
  }

  $effect(() => {
    if (!el) return;
    const r = el.getBoundingClientRect();
    const left = Math.max(6, Math.min(x, innerWidth - r.width - 6));
    const top = Math.max(6, Math.min(y, innerHeight - r.height - 6));
    pos = { left, top, origin: `${x - left}px ${y - top}px` };
    ready = true;
    el.querySelector<HTMLButtonElement>('button')?.focus();
  });

  function nav(e: KeyboardEvent) {
    const btns = [...(el?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = btns.indexOf(document.activeElement as HTMLButtonElement);
    let next: HTMLButtonElement | undefined;
    if (e.key === 'ArrowDown') next = btns[(i + 1) % btns.length];
    else if (e.key === 'ArrowUp') next = btns[(i - 1 + btns.length) % btns.length];
    else if (e.key === 'Escape' || e.key === 'Tab') onclose();
    else return;
    e.preventDefault();
    next?.focus();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="catch" out:gone|global onclick={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></div>
<div
  class="menu"
  role="menu"
  aria-label={label}
  tabindex="-1"
  bind:this={el}
  style:left="{pos.left}px"
  style:top="{pos.top}px"
  style:transform-origin={pos.origin}
  style:visibility={ready ? 'visible' : 'hidden'}
  in:menuPop|global
  out:menuPop|global={{ out: true }}
  onkeydown={nav}
  onfocusin={(e) => {
    const b = e.target as HTMLElement;
    if (b.matches(':focus-visible')) light(b);
  }}
  onpointerleave={unlight}
>
  <span class="hl" class:danger={litDanger} bind:this={hl} aria-hidden="true"></span>
  {#each items as it}
    <button
      type="button"
      role={it.checked === undefined ? 'menuitem' : 'menuitemradio'}
      aria-checked={it.checked}
      class:danger={it.danger}
      onpointerenter={(e) => light(e.currentTarget)}
      onclick={() => {
        onclose();
        it.run();
      }}
    >
      {#if it.checked !== undefined}
        <span class="slot">{#if it.checked}<Icon name="check" size={16} />{/if}</span>
      {:else if it.icon}<Icon name={it.icon} size={16} />{/if}
      {it.label}
    </button>
  {/each}
</div>

<style>
  .catch {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    z-index: 61;
    min-width: 184px;
    padding: var(--sp-1);
    border-radius: var(--r-md);
    background: var(--raised);
    box-shadow:
      0 0 0 1px var(--line-strong),
      var(--shadow-pop);
    outline: none;
  }
  .hl {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    border-radius: var(--r-sm);
    background: var(--hover);
    opacity: 0;
    pointer-events: none;
    transform-origin: 0 0;
    transition:
      opacity var(--t-fade) var(--ease-color),
      background-color var(--t-fade) var(--ease-color);
  }
  .hl.danger {
    background: var(--danger-wash);
  }
  button {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    height: var(--h-md);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--text);
    text-align: left;
    transition: color var(--t-fade) var(--ease-color);
  }
  button :global(.icon) {
    color: var(--text-2);
  }
  button:focus-visible {
    outline: none;
  }
  .danger,
  .danger :global(.icon) {
    color: var(--danger-text);
  }

  .slot {
    display: inline-flex;
    flex: none;
    width: 16px;
    color: var(--accent-text);
  }
</style>
