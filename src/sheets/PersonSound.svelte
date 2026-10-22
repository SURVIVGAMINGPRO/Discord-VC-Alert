<script lang="ts">
  // "Sound for {label}". Every edit saves at once; the file keeps v1's shape
  // ({beep: {…}} without gap_ratio, or {file}).
  import Sheet from '../components/Sheet.svelte';
  import Avatar from '../components/Avatar.svelte';
  import Button from '../components/Button.svelte';
  import SoundControls from '../components/SoundControls.svelte';
  import SpeakerIcon from '../components/SpeakerIcon.svelte';
  import Switch from '../components/Switch.svelte';
  import { app, avatars } from '../lib/app.svelte';
  import { call, errText } from '../lib/api';
  import { untrack } from 'svelte';
  import { describeBeep, keyOf, lockMs } from '../lib/util';
  import type { Beep, SoundSpec, Target } from '../lib/types';

  let { t, onclose }: { t: Target; onclose: () => void } = $props();
  // The sheet is opened for one person; their saved sound seeds the controls.
  const first = untrack(() => t);
  const key = keyOf(first);
  const cfg = $derived(app.snap!.config);
  const live = $derived(cfg.targets.find((x) => keyOf(x) === key) ?? first);

  type Mode = 'universal' | 'beep' | 'file';
  let mode = $state<Mode>(first.sound?.file ? 'file' : first.sound?.beep ? 'beep' : 'universal');
  let beep = $state<Beep>({ ...app.snap!.config.beep, ...(first.sound?.beep ?? {}) });
  let file = $state(first.sound?.file ?? '');

  function spec(m: Mode, b: Beep, f: string): SoundSpec | null {
    if (m === 'beep') return { beep: { frequency_hz: b.frequency_hz, duration_ms: b.duration_ms, repeats: b.repeats, volume: b.volume } };
    if (m === 'file' && f) return { file: f };
    return null;
  }

  // The leave sound: one softer, lower beep made from their sound.
  let leaveTesting = $state(false);
  let leaveError = $state('');
  async function testLeave() {
    if (leaveTesting) return;
    leaveError = '';
    try {
      const d = await call<number>('test_leave_sound', { key });
      leaveTesting = true;
      setTimeout(() => (leaveTesting = false), lockMs(d));
    } catch (e) {
      leaveError = errText(e);
    }
  }

  function change(m: Mode, b: Beep, f: string) {
    mode = m;
    beep = b;
    file = f;
    call('set_person_sound', { key, spec: spec(m, b, f) });
  }
</script>

<Sheet title="Sound for {live.label}" subtitle={first.username ? `@${first.username}` : `ID ${first.id}`} subtitleIsId width={460} {onclose}>
  {#snippet header()}
    <Avatar key={key} label={live.label} size={36} src={avatars[key] ?? null} />
  {/snippet}
  <SoundControls
    modes={[
      { value: 'universal', label: 'Universal' },
      { value: 'beep', label: 'Own beep' },
      { value: 'file', label: 'Sound file' },
    ]}
    {mode}
    {beep}
    {file}
    universalNote="{live.label} gets the universal sound ({cfg.sound_file ? 'a sound file' : describeBeep(cfg.beep)})."
    onchange={change}
    ontest={() => call<number>('test_sound', { spec: spec(mode, beep, file) })}
  />
  <div class="leave">
    <div class="leave-text">
      <span class="leave-title">Soft sound when they leave</span>
      <span class="leave-sub">One lower, quieter beep, so you know they've gone.</span>
    </div>
    <Button small onclick={testLeave} label="Test the leave sound"><SpeakerIcon size={16} playing={leaveTesting} /> Test</Button>
    <Switch label="Soft sound when they leave" checked={!!live.leave_sound} onchange={(v) => call('set_leave_sound', { key, on: v })} />
  </div>
  {#if leaveError}<p class="error" role="alert">{leaveError}</p>{/if}
  {#snippet footer()}
    <Button kind="ghost" disabled={mode === 'universal'} onclick={() => change('universal', beep, file)}>Use the universal sound</Button>
    <Button kind="primary" onclick={onclose}>Done</Button>
  {/snippet}
</Sheet>

<style>
  .leave {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: var(--sp-4);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--line);
  }
  .leave-text {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .leave-title {
    font-weight: var(--fw-medium);
  }
  .leave-sub,
  .error {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .error {
    margin: var(--sp-2) 0 0;
    color: var(--danger-text);
  }
</style>
