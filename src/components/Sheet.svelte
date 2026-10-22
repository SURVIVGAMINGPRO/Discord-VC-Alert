<script lang="ts">
  // A modal sheet, moving like Animate UI's dialog: it tips forward from the
  // top to upright, growing from --dialog-from out of a light blur on the
  // dialog spring (its fade ahead), and leaves the way it came on the dismiss
  // curve. The backdrop's tint and blur grow from nothing.
  import type { Snippet } from 'svelte';
  import IconButton from './IconButton.svelte';
  import { alertIn, alertOut, alertScrimIn, alertScrimOut } from '../lib/motion';
  let {
    title,
    subtitle,
    subtitleIsId = false,
    width = 440,
    onclose,
    children,
    footer,
    header,
  }: {
    title: string;
    subtitle?: string;
    subtitleIsId?: boolean;
    width?: number;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    header?: Snippet;
  } = $props();
  let box: HTMLDivElement | undefined = $state();
  const restore = typeof document !== 'undefined' ? (document.activeElement as HTMLElement | null) : null;
  $effect(() => {
    const first = box?.querySelector<HTMLElement>('[autofocus], input, textarea, select, button:not([aria-label="Close"])');
    (first ?? box)?.focus();
    return () => restore?.focus?.();
  });

  // Keep Tab inside the sheet.
  function trap(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
      return;
    }
    if (e.key !== 'Tab' || !box) return;
    const items = [...box.querySelectorAll<HTMLElement>('button, input, textarea, select, [tabindex="0"]')].filter(
      (x) => !x.hasAttribute('disabled') && x.offsetParent !== null,
    );
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="backdrop"
  in:alertScrimIn|global={{ color: '--scrim' }}
  out:alertScrimOut|global={{ color: '--scrim' }}
  onclick={onclose}
></div>
<div class="center">
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    bind:this={box}
    style:width="min({width}px, calc(100vw - var(--sp-6)))"
    in:alertIn|global
    out:alertOut|global
    onkeydown={trap}
  >
    <header>
      {@render header?.()}
      <div class="titles">
        <h2>{title}</h2>
        {#if subtitle}<p class:id={subtitleIsId}>{subtitle}</p>{/if}
      </div>
      <IconButton icon="close" label="Close" size={28} onclick={onclose} />
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: var(--scrim);
    backdrop-filter: blur(var(--blur-backdrop));
    -webkit-backdrop-filter: blur(var(--blur-backdrop));
  }
  .center {
    position: fixed;
    inset: 0;
    z-index: 51;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-3);
    pointer-events: none;
  }
  .sheet {
    display: flex;
    flex-direction: column;
    max-height: 100%;
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--line),
      var(--shadow-sheet);
    outline: none;
    pointer-events: auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-3) var(--sp-2) var(--sp-5);
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lg);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  p {
    margin: var(--sp-0) 0 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .body {
    padding: var(--sp-1) var(--sp-5) var(--sp-4);
    overflow: auto;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-5) var(--sp-4);
    border-top: 1px solid var(--line);
  }
</style>
