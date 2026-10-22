<script lang="ts">
  // The status lamp. The green ping repeats only while watching a channel,
  // pauses while the window is hidden, and is gone under reduced motion.
  let { tone, ping = false }: { tone: 'off' | 'warn' | 'live' | 'danger'; ping?: boolean } = $props();
  let visible = $state(typeof document === 'undefined' || document.visibilityState === 'visible');
</script>

<svelte:document onvisibilitychange={() => (visible = document.visibilityState === 'visible')} />
<span class="lamp {tone}" class:ping={ping && tone === 'live'} class:paused={!visible} aria-hidden="true"></span>

<style>
  .lamp {
    position: relative;
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: var(--r-full);
    background: var(--text-3);
    transition:
      background-color var(--t-fade) var(--ease-color),
      box-shadow var(--t-fade) var(--ease-color);
  }
  .warn {
    background: var(--warn);
    box-shadow: 0 0 0 4px var(--warn-wash);
  }
  .live {
    background: var(--live);
    box-shadow: 0 0 0 4px var(--live-wash);
  }
  .danger {
    background: var(--danger);
    box-shadow: 0 0 0 4px var(--danger-wash);
  }
  .ping::after {
    content: '';
    position: absolute;
    inset: -4px;
    border: 2px solid var(--live);
    border-radius: inherit;
    opacity: 0;
    animation: ping var(--t-ping) var(--ease-out) infinite;
  }
  .paused::after {
    animation-play-state: paused;
  }
  @keyframes ping {
    from {
      transform: scale(0.6);
      opacity: 0.9;
    }
    to {
      transform: scale(2.4);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ping::after {
      display: none;
    }
  }
</style>
