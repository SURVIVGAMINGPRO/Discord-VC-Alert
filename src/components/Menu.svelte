<script lang="ts">
  // A small menu that grows from where it was opened (the button or pointer).
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  import { gone, menuPop } from '../lib/motion';
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
    if (e.key === 'ArrowDown') btns[(i + 1) % btns.length]?.focus();
    else if (e.key === 'ArrowUp') btns[(i - 1 + btns.length) % btns.length]?.focus();
    else if (e.key === 'Escape' || e.key === 'Tab') onclose();
    else return;
    e.preventDefault();
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
>
  {#each items as it}
    <button
      type="button"
      role={it.checked === undefined ? 'menuitem' : 'menuitemradio'}
      aria-checked={it.checked}
      class:danger={it.danger}
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
  button {
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
    transition: background-color var(--t-fade) var(--ease-color);
  }
  button :global(.icon) {
    color: var(--text-2);
  }
  button:hover,
  button:focus-visible {
    background: var(--hover);
    outline: none;
  }
  .danger,
  .danger :global(.icon) {
    color: var(--danger-text);
  }
  .danger:hover,
  .danger:focus-visible {
    background: var(--danger-wash);
  }
  .slot {
    display: inline-flex;
    flex: none;
    width: 16px;
    color: var(--accent-text);
  }
</style>
