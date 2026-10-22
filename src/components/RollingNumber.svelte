<script lang="ts">
  // Digits roll up or down to the new value; only digits that change move.
  import { digit } from '../lib/motion';
  let { value }: { value: number } = $props();
  let dir = $state(1);
  let prev: number | undefined;
  $effect.pre(() => {
    dir = prev === undefined || value >= prev ? 1 : -1;
    prev = value;
  });
  const digits = $derived(String(Math.max(0, Math.round(value))).split(''));
</script>

<span class="roll num" aria-hidden="true">
  {#each digits as d, i (digits.length - i)}
    <span class="cell">
      {#key d}
        <span class="d" in:digit={{ dir }} out:digit={{ dir, out: true }}>{d}</span>
      {/key}
    </span>
  {/each}
</span>
<span class="sr-only">{value}</span>

<style>
  .roll {
    display: inline-flex;
    overflow: hidden;
    vertical-align: bottom;
    line-height: inherit;
  }
  .cell {
    position: relative;
    display: inline-block;
  }
  .d {
    display: inline-block;
  }
</style>
