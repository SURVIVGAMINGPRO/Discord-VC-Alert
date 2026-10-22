<script lang="ts">
  // The speaker icon. While `playing`, its sound waves gently pulse.
  import { ICONS } from '../lib/icons';
  let { size = 16, playing = false }: { size?: number; playing?: boolean } = $props();
</script>

<span class="spk" class:playing style:width="{size}px" style:height="{size}px" aria-hidden="true">
  <span class="body">{@html ICONS.volume_up}</span>
  <span class="waves">{@html ICONS.volume_up}</span>
</span>

<style>
  .spk {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  .spk span {
    position: absolute;
    inset: 0;
    display: flex;
  }
  .spk :global(svg) {
    width: 100%;
    height: 100%;
    fill: currentColor;
  }
  .body {
    clip-path: inset(0 45% 0 0);
  }
  .waves {
    clip-path: inset(0 0 0 55%);
  }
  .playing .waves {
    animation: waves var(--t-pulse) var(--ease-soft) infinite alternate;
  }
  @keyframes waves {
    to {
      opacity: 0.3;
    }
  }
</style>
