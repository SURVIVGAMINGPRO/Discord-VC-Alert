<script lang="ts">
  // A confirmation (Animate UI's alert dialog): a title, what will happen,
  // then Cancel and the action. Cancel has focus first, Escape cancels, and
  // Tab stays inside. Narrow windows centre the text and split the buttons;
  // wide ones line them up on the right.
  import Button from './Button.svelte';
  import { alertIn, alertOut, alertScrimIn, alertScrimOut } from '../lib/motion';

  let {
    title,
    description,
    confirm,
    cancel = 'Cancel',
    danger = false,
    onconfirm,
    oncancel,
  }: {
    title: string;
    description: string;
    confirm: string;
    cancel?: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();

  const id = `alert-${Math.random().toString(36).slice(2, 8)}`;
  let box: HTMLDivElement | undefined = $state();
  const restore = typeof document !== 'undefined' ? (document.activeElement as HTMLElement | null) : null;
  $effect(() => {
    box?.querySelector<HTMLElement>('[data-cancel] button, button')?.focus();
    return () => restore?.focus?.();
  });

  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      oncancel();
      return;
    }
    if (e.key !== 'Tab' || !box) return;
    const items = [...box.querySelectorAll<HTMLElement>('button')].filter((x) => !x.hasAttribute('disabled'));
    const i = items.indexOf(document.activeElement as HTMLElement);
    e.preventDefault();
    items[(i + (e.shiftKey ? items.length - 1 : 1)) % items.length]?.focus();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" in:alertScrimIn|global out:alertScrimOut|global onclick={oncancel}></div>
<div class="center">
  <div
    class="card"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="{id}-t"
    aria-describedby="{id}-d"
    tabindex="-1"
    bind:this={box}
    in:alertIn|global
    out:alertOut|global
    onkeydown={keys}
  >
    <div class="head">
      <h2 id="{id}-t">{title}</h2>
      <p id="{id}-d">{description}</p>
    </div>
    <div class="foot">
      <span data-cancel><Button full onclick={oncancel}>{cancel}</Button></span>
      <Button full kind={danger ? 'danger' : 'primary'} onclick={onconfirm}>{confirm}</Button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: var(--alert-scrim);
    backdrop-filter: blur(var(--blur-backdrop));
    -webkit-backdrop-filter: blur(var(--blur-backdrop));
  }
  .center {
    position: fixed;
    inset: 0;
    z-index: 81;
    display: grid;
    place-items: center;
    padding: var(--sp-4);
    pointer-events: none;
  }
  .card {
    display: grid;
    gap: var(--sp-4);
    width: min(320px, 100%);
    padding: var(--sp-4);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--line),
      var(--shadow-sheet);
    outline: none;
    pointer-events: auto;
    overflow: hidden;
  }
  .head {
    display: grid;
    gap: var(--sp-1);
    text-align: center;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-medium);
    line-height: var(--lh-tight);
  }
  p {
    margin: 0;
    color: var(--text-2);
    text-wrap: balance;
  }
  /* The footer runs edge to edge on a slightly darker band. */
  .foot {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-2);
    margin: 0 calc(-1 * var(--sp-4)) calc(-1 * var(--sp-4));
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  .foot > span {
    display: grid;
  }
  @media (min-width: 640px) {
    .card {
      width: 384px;
    }
    .head {
      text-align: left;
    }
    p {
      text-wrap: pretty;
    }
    .foot {
      display: flex;
      justify-content: flex-end;
    }
    .foot :global(.btn) {
      width: auto;
    }
  }
</style>
