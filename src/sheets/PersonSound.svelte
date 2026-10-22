<script lang="ts">
  // "Sound for {label}": what plays when they join, and when they leave. Every
  // edit saves at once, in the watched list only (each list keeps its own).
  // The join sound keeps v1's shape ({beep: {…}} without gap_ratio, or {file}).
  import Sheet from '../components/Sheet.svelte';
  import Avatar from '../components/Avatar.svelte';
  import Button from '../components/Button.svelte';
  import SoundControls from '../components/SoundControls.svelte';
  import { app, avatars } from '../lib/app.svelte';
  import { call } from '../lib/api';
  import { untrack } from 'svelte';
  import { describeBeep, keyOf, leaveBeep } from '../lib/util';
  import type { Beep, SoundSpec, Target } from '../lib/types';

  let { t, onclose }: { t: Target; onclose: () => void } = $props();
  // The sheet is opened for one person; their saved sounds seed the controls.
  const first = untrack(() => t);
  const key = keyOf(first);
  const cfg = $derived(app.snap!.config);
  const live = $derived(cfg.targets.find((x) => keyOf(x) === key) ?? first);

  type Mode = 'off' | 'universal' | 'beep' | 'file';
  const universalBeep = untrack(() => app.snap!.config.beep);

  // When they join.
  let mode = $state<Mode>(first.sound?.file ? 'file' : first.sound?.beep ? 'beep' : 'universal');
  let beep = $state<Beep>({ ...universalBeep, ...(first.sound?.beep ?? {}) });
  let file = $state(first.sound?.file ?? '');

  // When they leave ('universal' is the soft beep made from their sound).
  let leaveMode = $state<Mode>(
    first.leave_sound === false ? 'off' : first.leave?.file ? 'file' : first.leave?.beep ? 'beep' : 'universal',
  );
  let leaveBeepNow = $state<Beep>(
    first.leave?.beep ? { ...universalBeep, ...first.leave.beep } : untrack(() => leaveBeep(beep)),
  );
  let leaveFile = $state(first.leave?.file ?? '');

  function spec(m: Mode, b: Beep, f: string): SoundSpec | null {
    if (m === 'beep') return { beep: { frequency_hz: b.frequency_hz, duration_ms: b.duration_ms, repeats: b.repeats, volume: b.volume } };
    if (m === 'file' && f) return { file: f };
    return null;
  }

  function change(m: Mode, b: Beep, f: string) {
    mode = m;
    beep = b;
    file = f;
    call('set_person_sound', { key, spec: spec(m, b, f) });
  }

  function changeLeave(m: Mode, b: Beep, f: string) {
    leaveMode = m;
    leaveBeepNow = b;
    leaveFile = f;
    call('set_leave', { key, on: m !== 'off', spec: spec(m, b, f) });
  }
</script>

<Sheet title="Sound for {live.label}" subtitle={first.username ? `@${first.username}` : `ID ${first.id}`} subtitleIsId width={460} {onclose}>
  {#snippet header()}
    <Avatar key={key} label={live.label} size={36} src={avatars[key] ?? null} />
  {/snippet}
  <h3>When they join</h3>
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

  <h3 class="leave">When they leave</h3>
  {#if !cfg.general.leave_sounds}
    <p class="note">Leave sounds are off for everyone in Settings → Sound. What you choose here plays once they're on.</p>
  {/if}
  <SoundControls
    modes={[
      { value: 'off', label: 'Off' },
      { value: 'universal', label: 'Soft' },
      { value: 'beep', label: 'Own beep' },
      { value: 'file', label: 'Sound file' },
    ]}
    mode={leaveMode}
    beep={leaveBeepNow}
    file={leaveFile}
    universalNote="One lower, quieter beep made from their join sound."
    offNote="No sound when {live.label} leaves."
    onchange={changeLeave}
    ontest={() => call<number>('test_leave_sound', { key })}
  />
  {#snippet footer()}
    <Button kind="ghost" disabled={mode === 'universal'} onclick={() => change('universal', beep, file)}>Use the universal sound</Button>
    <Button kind="primary" onclick={onclose}>Done</Button>
  {/snippet}
</Sheet>

<style>
  h3 {
    margin: 0 0 var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-medium);
  }
  h3.leave {
    margin-top: var(--sp-5);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--line);
  }
  .note {
    margin: 0 0 var(--sp-3);
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
</style>
