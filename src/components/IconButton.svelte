<script lang="ts">
  // A square icon button. When `icon` changes, the icons swap in place (scale
  // from 0.5 out of a light blur). `tone="accent"` keeps it tinted, e.g. the
  // speaker of someone with their own sound.
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  import { iconIn, iconOut } from '../lib/motion';
  let {
    icon,
    label,
    active = false,
    size = 30,
    flat = true,
    tone,
    disabled = false,
    tabindex,
    onclick,
  }: {
    icon: IconName;
    label: string;
    active?: boolean;
    size?: number;
    flat?: boolean;
    tone?: 'accent';
    disabled?: boolean;
    tabindex?: number;
    onclick?: (e: MouseEvent) => void;
  } = $props();
</script>

<button
  type="button"
  class="ib"
  class:active
  class:boxed={!flat}
  class:accent={tone === 'accent'}
  aria-label={label}
  title={label}
  {disabled}
  {tabindex}
  style:width="{size}px"
  style:height="{size}px"
  {onclick}
>
  {#key icon}
    <span class="glyph" in:iconIn out:iconOut><Icon name={icon} size={Math.max(16, Math.round(size * 0.6))} /></span>
  {/key}
</button>

<style>
  .ib {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    padding: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    transition:
      background-color var(--t-fade) var(--ease-color),
      color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .boxed {
    box-shadow: inset 0 0 0 1px var(--line-strong);
    background: var(--raised);
  }
  .ib:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .ib:active:not(:disabled) {
    transform: scale(var(--press-scale));
  }
  .accent {
    color: var(--accent-text);
  }
  .accent:hover:not(:disabled) {
    color: var(--accent-text);
  }
  .active,
  .active:hover:not(:disabled) {
    background: var(--accent-wash);
    color: var(--accent-text);
  }
  .ib:disabled {
    color: var(--text-3);
    opacity: 0.6;
    cursor: default;
  }
  @media (prefers-reduced-motion: reduce) {
    .ib:active:not(:disabled) {
      transform: none;
    }
  }
</style>
