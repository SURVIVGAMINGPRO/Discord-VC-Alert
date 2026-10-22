<script lang="ts">
  import { avatarColor, initialOf } from '../lib/util';
  let {
    key,
    label,
    size = 30,
    src = null,
    present = false,
  }: { key: string; label: string; size?: number; src?: string | null; present?: boolean } = $props();
</script>

<span class="av" style:width="{size}px" style:height="{size}px">
  {#if src}
    <img {src} alt="" draggable="false" />
  {:else}
    <span class="letter" style:background={avatarColor(key)} style:font-size="{Math.max(8, Math.round(size * 0.44))}px">
      {#if size >= 14}{initialOf(label)}{/if}
    </span>
  {/if}
  {#if present}<span class="dot" style:width="{Math.max(9, Math.round(size * 0.36))}px" style:height="{Math.max(9, Math.round(size * 0.36))}px"></span>{/if}
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
