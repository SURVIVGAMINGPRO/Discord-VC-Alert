<script lang="ts">
  // Discord app details: used by Settings → Connection and the first-run wizard.
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import TextField from './TextField.svelte';
  import Icon from './Icon.svelte';
  import { app } from '../lib/app.svelte';
  import { call, errText } from '../lib/api';
  import { fold, shake, swapIn, unfold } from '../lib/motion';

  let { onsaved, compact = false }: { onsaved?: () => void; compact?: boolean } = $props();

  const snap = $derived(app.snap!);
  let clientId = $state(app.snap!.config.client_id);
  let secret = $state('');
  let showSecret = $state(false);
  let help = $state(false);
  let saved = $state(false);
  let error = $state('');
  let idBox: HTMLDivElement | undefined = $state();
  let testState = $state<'idle' | 'testing' | 'ok' | 'fail'>('idle');
  let testMsg = $state('');

  async function reveal() {
    if (!showSecret && !secret && snap.has_secret) secret = await call<string>('reveal_secret');
    showSecret = !showSecret;
  }

  async function save() {
    error = '';
    if (!/^\d{15,25}$/.test(clientId.trim())) {
      error = 'The Application ID is a long number. Copy it from the Developer Portal.';
      shake(idBox);
      return;
    }
    if (!secret.trim() && !snap.has_secret) {
      error = 'Paste the Client Secret too.';
      return;
    }
    try {
      await call('save_credentials', { clientId: clientId.trim(), clientSecret: secret.trim() || null });
      saved = true;
      setTimeout(() => (saved = false), 2500);
      onsaved?.();
    } catch (e) {
      error = errText(e);
      shake(idBox);
    }
  }

  async function test() {
    testState = 'testing';
    testMsg = '';
    try {
      const name = await call<string>('test_connection');
      testState = 'ok';
      testMsg = name ? `Connected as @${name}` : 'Connected';
    } catch (e) {
      testState = 'fail';
      testMsg = errText(e);
    }
  }

  const testLabel = $derived(
    testState === 'testing' ? 'Testing…' : testState === 'ok' ? testMsg : 'Test connection',
  );
</script>

<div class="creds" class:compact>
  <span class="lbl">Application ID</span>
  <div bind:this={idBox}>
    <TextField
      label="Application ID"
      placeholder="123456789012345678"
      mono
      invalid={!!error && error.startsWith('The Application ID')}
      describedby="cred-msg"
      bind:value={clientId}
      oninput={() => (error = '')}
      onenter={save}
    />
  </div>
  <span class="lbl">Client Secret</span>
  <TextField
    label="Client Secret"
    placeholder={snap.has_secret ? 'Saved. Paste a new one to replace it' : 'Client Secret'}
    password={!showSecret}
    bind:value={secret}
    onenter={save}
  >
    {#snippet trailing()}
      <IconButton
        icon={showSecret ? 'visibility_off' : 'visibility'}
        label={showSecret ? 'Hide the secret' : 'Show the secret'}
        size={28}
        onclick={reveal}
      />
    {/snippet}
  </TextField>

  <div class="actions">
    <Button kind="link" small onclick={() => (help = !help)}>{help ? 'Hide the steps' : 'Where do I get these?'}</Button>
    <span class="grow"></span>
    <Button kind="primary" small swap={saved ? 'saved' : 'save'} onclick={save}>{saved ? 'Saved' : 'Save'}</Button>
  </div>
  <p class="msg" class:error={!!error} id="cred-msg" aria-live="polite">{error}</p>

  {#if help}
    <div class="help" in:unfold out:fold>
      <ol>
        <li>Open the Discord Developer Portal and choose <b>New Application</b>. Any boring name works.</li>
        <li>Copy the <b>Application ID</b> into the field above.</li>
        <li>Under <b>OAuth2</b>, choose <b>Reset Secret</b> and copy the <b>Client Secret</b>.</li>
        <li>Under <b>OAuth2 → Redirects</b>, add <b>http://localhost</b> and save.</li>
        <li>If there's an <b>App Testers</b> section, add your own account.</li>
      </ol>
      <Button small onclick={() => call('open_developer_portal')}>Open the Developer Portal</Button>
    </div>
  {/if}

  <div class="test">
    <Button small swap={testState === 'ok' ? testMsg : testState} disabled={testState === 'testing'} onclick={test}>{#if testState === 'ok'}<span class="ok"><Icon name="check" size={16} /></span>{/if}{testLabel}</Button>
    {#if testState === 'fail'}
      {#key testMsg}<p class="msg error" in:swapIn role="alert">{testMsg}</p>{/key}
    {:else}
      <p class="msg">Connects to the Discord app once to check everything. The first time, approve the popup inside Discord.</p>
    {/if}
  </div>
</div>

<style>
  .creds {
    display: grid;
    gap: var(--sp-1);
  }
  .lbl {
    margin-top: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  .lbl:first-child {
    margin-top: 0;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
  }
  .grow {
    flex: 1;
  }
  .msg {
    margin: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .msg:empty {
    display: none;
  }
  .msg.error {
    color: var(--danger-text);
  }
  .help {
    padding: var(--sp-3);
    border-radius: var(--r-md);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .help ol {
    margin: 0 0 var(--sp-3);
    padding-left: var(--sp-5);
    color: var(--text-2);
    font-size: var(--fs-sm);
    display: grid;
    gap: var(--sp-1);
  }
  .help b {
    color: var(--text);
    font-weight: var(--fw-medium);
  }
  .ok {
    display: inline-flex;
    color: var(--live-text);
  }
  .test {
    display: grid;
    justify-items: start;
    gap: var(--sp-1);
    margin-top: var(--sp-3);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
</style>
