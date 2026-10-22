<script lang="ts">
  // Toasts rise from the bottom out of a light blur and leave with --ease-away.
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  import { toasts, hideToast } from '../lib/app.svelte';
  import { toastIn, toastOut } from '../lib/motion';
  let { bottom = 16 }: { bottom?: number } = $props();
  const ICON: Record<string, IconName> = { ok: 'check', error: 'warning', info: 'info' };
</script>

<div class="region" style:bottom="{bottom}px" role="status" aria-live="polite">
  {#if toasts.current}
    {@const t = toasts.current}
    {#key t.id}
      <div class="toast {t.kind}" in:toastIn|global out:toastOut|global>
        <span class="ic"><Icon name={ICON[t.kind]} size={16} /></span>
        <span class="msg">{t.message}</span>
        {#if t.action}
          <button
            type="button"
            onclick={() => {
              const run = t.action!.run;
              hideToast();
              run();
            }}>{t.action.label}</button
          >
        {/if}
      </div>
    {/key}
  {/if}
</div>

<style>
  .region {
    position: fixed;
    left: 0;
    right: 0;
    z-index: 70;
    display: grid;
    justify-items: center;
    padding: 0 var(--sp-4);
    pointer-events: none;
  }
  .toast {
    grid-area: 1 / 1;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    max-width: 440px;
    min-width: 0;
    padding: var(--sp-2) var(--sp-2) var(--sp-2) var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    pointer-events: auto;
  }
  .ic {
    display: flex;
    flex: none;
  }
  .ok .ic {
    color: var(--live);
  }
  .error .ic {
    color: var(--danger);
  }
  .info .ic {
    color: var(--accent-text);
  }
  .msg {
    flex: 1;
    min-width: 0;
    padding: var(--sp-1) 0;
  }
  button {
    flex: none;
    height: var(--h-sm);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--accent-text);
    font-weight: var(--fw-bold);
    transition:
      background-color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  button:hover {
    background: var(--accent-wash);
  }
  button:active {
    transform: scale(var(--press-scale));
  }
</style>
