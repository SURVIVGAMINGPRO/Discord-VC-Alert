<script lang="ts">
  // A tiny picture of the beep pattern: one bar per beep, gaps scaled to length.
  import type { Beep } from '../lib/types';
  let { beep }: { beep: Beep } = $props();
  const W = 132;
  const H = 18;
  const bars = $derived.by(() => {
    const gap = beep.duration_ms * beep.gap_ratio;
    const total = beep.repeats * beep.duration_ms + (beep.repeats - 1) * gap;
    const scale = W / Math.max(total, 1);
    const h = 4 + (H - 8) * beep.volume;
    return Array.from({ length: beep.repeats }, (_, i) => ({
      x: i * (beep.duration_ms + gap) * scale,
      w: Math.max(3, beep.duration_ms * scale),
      h,
    }));
  });
</script>

<svg width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true">
  <line x1="0" y1={H / 2} x2={W} y2={H / 2} stroke="var(--line-strong)" />
  {#each bars as b}
    <rect x={b.x} y={(H - b.h) / 2} width={b.w} height={b.h} rx={Math.min(3, b.h / 2)} fill="var(--accent)" opacity="0.85" />
  {/each}
</svg>
