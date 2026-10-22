<script lang="ts">
  // "Your profiles": up to 3 saved beeps with names of up to 12 characters,
  // stored in settings.json (beep_profiles). Shared by Settings → Sound and
  // the per-person sound sheet.
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import TextField from './TextField.svelte';
  import { app, toast } from '../lib/app.svelte';
  import { call, errText } from '../lib/api';
  import { fold, shake, unfold } from '../lib/motion';
  import { BEEP_PRESETS, MAX_PROFILES, PROFILE_NAME_MAX, sameBeep } from '../lib/util';
  import type { Beep, BeepProfile } from '../lib/types';

  let { beep, onpick }: { beep: Beep; onpick: (b: BeepProfile['beep']) => void } = $props();

  const profiles = $derived(app.snap?.config.beep_profiles ?? []);
  const same = (b: BeepProfile['beep']) => sameBeep(b, beep);
  const saved = $derived(profiles.some((p) => same(p.beep)) || BEEP_PRESETS.some((p) => same(p.beep)));
  const full = $derived(profiles.length >= MAX_PROFILES);

  let naming = $state(false);
  let name = $state('');
  let error = $state('');
  let box: HTMLDivElement | undefined = $state();
  let field: HTMLInputElement | undefined = $state();

  // autofocus only works on page load, so focus the name field when it opens.
  $effect(() => {
    if (naming && field) field.focus();
  });

  function open() {
    naming = !naming;
    name = '';
    error = '';
  }

  // Send plain data, not Svelte's reactive proxies of the current list.
  const store = (next: BeepProfile[]) => call('update_settings', { patch: { beep_profiles: $state.snapshot(next) } });

  async function save() {
    const n = name.trim();
    if (!n) {
      error = 'Give the profile a name.';
      shake(box);
      return;
    }
    if (profiles.some((p) => p.name.toLowerCase() === n.toLowerCase())) {
      error = `You already have a profile called “${n}”.`;
      shake(box);
      return;
    }
    const b = { frequency_hz: beep.frequency_hz, duration_ms: beep.duration_ms, repeats: beep.repeats, volume: beep.volume };
    try {
      await store([...profiles, { name: [...n].slice(0, PROFILE_NAME_MAX).join(''), beep: b }]);
      naming = false;
      toast('ok', `Saved profile “${n}”`);
    } catch (e) {
      error = errText(e);
    }
  }

  async function remove(i: number) {
    const p = profiles[i];
    try {
      await store(profiles.filter((_, j) => j !== i));
      toast('info', `Deleted profile “${p.name}”`, {
        action: {
          label: 'Undo',
          run: () => {
            const now = app.snap?.config.beep_profiles ?? [];
            if (now.length >= MAX_PROFILES) return toast('error', 'You already have 3 profiles again. Delete one first.');
            const back = [...now];
            back.splice(Math.min(i, back.length), 0, p);
            store(back);
          },
        },
      });
    } catch (e) {
      toast('error', errText(e));
    }
  }
</script>

<div class="profiles">
  <div class="row" role="group" aria-label="Your beep profiles">
    <span class="cap">Your profiles</span>
    {#each profiles as p, i (p.name)}
      <span class="chip" class:on={same(p.beep)}>
        <button
          type="button"
          class="pick"
          aria-pressed={same(p.beep)}
          title="{p.beep.frequency_hz} Hz · {p.beep.duration_ms} ms · {p.beep.repeats}× · {Math.round(p.beep.volume * 100)}%"
          onclick={() => onpick(p.beep)}>{p.name}</button
        >
        <button type="button" class="del" aria-label="Delete profile {p.name}" title="Delete profile" onclick={() => remove(i)}>
          <Icon name="close" size={14} />
        </button>
      </span>
    {/each}
    {#if !saved}
      <button
        type="button"
        class="save"
        disabled={full}
        aria-expanded={naming}
        title={full ? 'You can keep 3 profiles. Delete one to save this beep.' : 'Save this beep as a profile'}
        onclick={open}
      >
        <Icon name="add" size={14} /> Save as profile
      </button>
    {/if}
  </div>

  {#if !saved && full}
    <p class="note">You can keep 3 profiles. Delete one to save this beep.</p>
  {/if}

  {#if naming && !full}
    <div class="naming" in:unfold out:fold>
      <div class="naming-in">
        <div class="field" bind:this={box}>
          <TextField
            label="Profile name"
            placeholder="Profile name"
            maxlength={PROFILE_NAME_MAX}
            bind:input={field}
            invalid={!!error}
            describedby="profile-msg"
            bind:value={name}
            oninput={() => (error = '')}
            onenter={save}
            onescape={() => (naming = false)}
          />
        </div>
        <span class="count num" aria-hidden="true">{[...name].length}/{PROFILE_NAME_MAX}</span>
        <Button small kind="ghost" onclick={() => (naming = false)}>Cancel</Button>
        <Button small kind="primary" onclick={save}>Save</Button>
      </div>
      <p class="msg" class:error={!!error} id="profile-msg" aria-live="polite">{error}</p>
    </div>
  {/if}
</div>

<style>
  .profiles {
    display: grid;
    gap: var(--sp-1);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
  }
  .cap {
    margin-right: var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: var(--h-sm);
    border-radius: var(--r-sm);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
    transition: background-color var(--t-fade) var(--ease-color);
  }
  .chip.on {
    background: var(--accent-wash);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .pick,
  .del,
  .save {
    border: 0;
    background: none;
    transition:
      color var(--t-fade) var(--ease-color),
      background-color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .pick {
    height: 100%;
    padding: 0 var(--sp-1) 0 var(--sp-2);
    border-radius: var(--r-sm) 0 0 var(--r-sm);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .chip.on .pick {
    color: var(--accent-text);
  }
  .pick:hover {
    color: var(--text);
  }
  .del {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--h-sm);
    height: 100%;
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
    color: var(--text-3);
  }
  .del:hover {
    color: var(--danger-text);
    background: var(--danger-wash);
  }
  .save {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    height: var(--h-sm);
    padding: 0 var(--sp-2);
    border-radius: var(--r-sm);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--accent-text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  .save:hover:not(:disabled) {
    background: var(--accent-wash);
  }
  .save:disabled {
    color: var(--text-3);
    cursor: default;
  }
  .pick:active,
  .save:active:not(:disabled) {
    transform: scale(var(--press-scale));
  }
  .naming-in {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding-top: var(--sp-1);
  }
  .field {
    flex: 1;
    min-width: 0;
  }
  .count {
    flex: none;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .note,
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
  @media (prefers-reduced-motion: reduce) {
    .pick:active,
    .save:active:not(:disabled) {
      transform: none;
    }
  }
</style>
