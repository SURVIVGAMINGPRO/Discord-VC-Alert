<script lang="ts">
  let { checked, label, onchange, disabled = false }: { checked: boolean; label: string; onchange: (v: boolean) => void; disabled?: boolean } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  class="sw"
  class:on={checked}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="thumb"></span>
</button>

<style>
  .sw {
    position: relative;
    flex: none;
    width: 36px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: var(--r-full);
    background: var(--line-strong);
    transition:
      background-color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .sw.on {
    background: var(--live);
  }
  .sw:active:not(:disabled) {
    transform: scale(var(--press-scale));
  }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: var(--r-full);
    background: var(--on-accent);
    box-shadow: var(--shadow-pop);
    transition: transform var(--spring-thumb-dur) var(--spring-thumb-ease);
  }
  .on .thumb {
    transform: translateX(16px);
  }
  .sw:disabled {
    opacity: 0.5;
    cursor: default;
  }
  @media (prefers-reduced-motion: reduce) {
    .thumb {
      transition: none;
    }
    .sw:active:not(:disabled) {
      transform: none;
    }
  }
</style>
