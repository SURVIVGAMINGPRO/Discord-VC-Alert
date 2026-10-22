<script lang="ts">
  import { onMount } from 'svelte';
  import { app, applyAppearance, initState } from './lib/app.svelte';
  import { windowLabel } from './lib/api';
  import Main from './views/Main.svelte';
  import Settings from './views/Settings.svelte';
  import Overlay from './views/Overlay.svelte';
  import EditorGhost from './views/EditorGhost.svelte';
  import EditorPanel from './views/EditorPanel.svelte';

  let label = $state<string | null>(null);

  onMount(() => {
    windowLabel().then(async (l) => {
      document.documentElement.dataset.view = l;
      await initState(l);
      label = l;
    });
    const mq = matchMedia('(prefers-color-scheme: dark)');
    const sync = () => app.snap && applyAppearance(app.snap.config.appearance.theme, app.snap.config.appearance.density);
    mq.addEventListener('change', sync);
    return () => mq.removeEventListener('change', sync);
  });

  $effect(() => {
    if (app.snap) applyAppearance(app.snap.config.appearance.theme, app.snap.config.appearance.density);
  });
</script>

{#if app.snap && label}
  {#if label === 'settings'}
    <Settings />
  {:else if label === 'overlay'}
    <Overlay />
  {:else if label === 'editor-ghost'}
    <EditorGhost />
  {:else if label === 'editor-panel'}
    <EditorPanel />
  {:else}
    <Main />
  {/if}
{/if}
