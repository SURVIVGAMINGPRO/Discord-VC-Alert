<script lang="ts">
  // First run: welcome → Discord app details → check the connection → people.
  // Each step slides 24 px sideways in the direction you're going.
  import Sheet from '../components/Sheet.svelte';
  import Button from '../components/Button.svelte';
  import CredentialsForm from '../components/CredentialsForm.svelte';
  import { app } from '../lib/app.svelte';
  import { call } from '../lib/api';
  import { step as slide } from '../lib/motion';

  const snap = $derived(app.snap!);

  const STEPS = ['welcome', 'details', 'people'] as const;
  let i = $state(0);
  let dir = $state(1);
  const go = (to: number) => {
    dir = to > i ? 1 : -1;
    i = to;
  };
  const finish = () => call('complete_setup');

  const ready = $derived(!!snap.config.client_id && snap.has_secret);
</script>

<Sheet
  title={i === 0 ? 'Welcome to VC Alert' : i === 1 ? 'Connect to Discord' : 'Who should VC Alert watch?'}
  subtitle="Step {i + 1} of {STEPS.length}"
  width={480}
  onclose={finish}
>
  <div class="steps">
    {#key i}
      <div class="step" in:slide={{ dir }} out:slide={{ dir, out: true }}>
        {#if STEPS[i] === 'welcome'}
          <p>VC Alert gives you a quiet beep, and a tiny picture in the corner, when people you choose join the Discord voice channel you're in.</p>
          <p class="muted">It talks only to the Discord app on this computer. Nothing is posted anywhere and nobody else can see it.</p>
          <div class="choices">
            <Button kind="primary" onclick={() => go(1)}>Set up</Button>
          </div>
        {:else if STEPS[i] === 'details'}
          <p class="muted">VC Alert needs a Discord application of your own. It takes about two minutes and nobody else sees it.</p>
          <CredentialsForm compact />
        {:else}
          <p>Type a username in the search box on the main window and press Enter to add someone. You can also paste a list or import a file from the add menu.</p>
          <p class="muted">Use their unique username, not the name they show in the server.</p>
        {/if}
      </div>
    {/key}
  </div>

  {#snippet footer()}
    {#if i > 0}<Button kind="ghost" onclick={() => go(i - 1)}>Back</Button>{/if}
    <span class="grow"></span>
    {#if i === 0}
      <Button kind="ghost" onclick={finish}>Skip for now</Button>
    {:else if i === 1}
      <Button kind={ready ? 'primary' : 'secondary'} swap={ready ? 'ready' : 'later'} onclick={() => go(2)}>{ready ? 'Continue' : 'Do this later'}</Button>
    {:else}
      <Button kind="primary" onclick={finish}>Start using VC Alert</Button>
    {/if}
  {/snippet}
</Sheet>

<style>
  .steps {
    position: relative;
    min-height: 220px;
  }
  .step {
    display: grid;
    gap: var(--sp-3);
    align-content: start;
  }
  p {
    margin: 0;
    max-width: 60ch;
  }
  .muted {
    color: var(--text-2);
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .grow {
    flex: 1;
  }
</style>
