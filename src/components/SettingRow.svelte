<script lang="ts">
  import type { Snippet } from 'svelte';
  let { title, sub, children, stack = false }: { title: string; sub?: string; children: Snippet; stack?: boolean } = $props();
</script>

<div class="row" class:stack>
  <div class="text">
    <div class="title">{title}</div>
    {#if sub}<div class="sub">{sub}</div>{/if}
  </div>
  <div class="ctl">{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) 0;
  }
  .row + :global(.row) {
    border-top: 1px solid var(--line);
  }
  .stack {
    flex-direction: column;
    align-items: stretch;
    gap: var(--sp-2);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    color: var(--text);
  }
  .sub {
    margin-top: var(--sp-0);
    max-width: 52ch;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .ctl {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-2);
  }
  .stack .ctl {
    flex: auto;
    flex-direction: column;
    align-items: stretch;
  }
</style>
