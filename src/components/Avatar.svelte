<script lang="ts">
  import { avatarColor, initialOf } from '../lib/util';
  // `size` is pixels, or a CSS length such as "var(--avatar)" so the picture
  // follows the density setting.
  let {
    key,
    label,
    size = 30,
    src = null,
    present = false,
  }: { key: string; label: string; size?: number | string; src?: string | null; present?: boolean } = $props();

  const box = $derived(typeof size === 'number' ? `${size}px` : size);
  const letter = $derived(typeof size === 'number' ? `${Math.max(8, Math.round(size * 0.44))}px` : `calc(${size} * 0.44)`);
  const dot = $derived(typeof size === 'number' ? `${Math.max(9, Math.round(size * 0.36))}px` : `max(9px, calc(${size} * 0.36))`);
  const showInitial = $derived(typeof size !== 'number' || size >= 14);
</script>

<span class="av" style:width={box} style:height={box}>
  {#if src}
    <img {src} alt="" draggable="false" />
  {:else}
    <span class="letter" style:background={avatarColor(key)} style:font-size={letter}>
      {#if showInitial}{initialOf(label)}{/if}
    </span>
  {/if}
  {#if present}<span class="dot" style:width={dot} style:height={dot}></span>{/if}
</span>

<style>
  .av {
    position: relative;
    display: inline-flex;
    flex: none;
    border-radius: var(--r-full);
  }
  img,
  .letter {
    width: 100%;
    height: 100%;
    border-radius: var(--r-full);
    object-fit: cover;
  }
  .letter {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--on-accent);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
  .dot {
    position: absolute;
    right: -2px;
    bottom: -2px;
    border-radius: var(--r-full);
    background: var(--live);
    box-shadow: 0 0 0 2.5px var(--dot-ring, var(--bg));
    transition: box-shadow var(--t-fade) var(--ease-color);
  }
</style>
