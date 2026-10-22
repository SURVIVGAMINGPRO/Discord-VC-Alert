<script lang="ts">
  // Shared by Settings → Sound (Generated beep / Sound file) and the
  // per-person sound sheet (Universal / Custom beep / Sound file).
  import Segmented from './Segmented.svelte';
  import Slider from './Slider.svelte';
  import Button from './Button.svelte';
  import SpeakerIcon from './SpeakerIcon.svelte';
  import Waveform from './Waveform.svelte';
  import { errText, pickFile, call } from '../lib/api';
  import { BEEP_PRESETS, baseName, lockMs } from '../lib/util';
  import { play, reduced, swapIn } from '../lib/motion';
  import type { Beep } from '../lib/types';

  type Mode = 'universal' | 'beep' | 'file';
  let {
    modes,
    mode,
    beep,
    file,
    universalNote = '',
    onchange,
    ontest,
  }: {
    modes: { value: Mode; label: string }[];
    mode: Mode;
    beep: Beep;
    file: string;
    universalNote?: string;
    onchange: (mode: Mode, beep: Beep, file: string) => void;
    ontest: () => Promise<number>;
  } = $props();

  let playing = $state(false);
  let error = $state('');
  let page: HTMLDivElement | undefined = $state();
  let before = 0;

  async function test() {
    if (playing) return;
    error = '';
    try {
      const d = await ontest();
      playing = true;
      setTimeout(() => (playing = false), lockMs(d));
    } catch (e) {
      error = errText(e);
    }
  }

  async function browse() {
    const path = await pickFile('Choose a sound', [
      { name: 'Sound (WAV, MP3, OGG, FLAC)', extensions: ['wav', 'mp3', 'ogg', 'flac'] },
    ]);
    if (!path) return;
    try {
      const stored = await call<string>('choose_sound_file', { path });
      error = '';
      onchange('file', beep, stored);
    } catch (e) {
      error = errText(e);
    }
  }

  const set = (k: keyof Beep, v: number) => onchange(mode, { ...beep, [k]: v }, file);
  const isPreset = (p: (typeof BEEP_PRESETS)[number]) =>
    p.beep.frequency_hz === beep.frequency_hz &&
    p.beep.duration_ms === beep.duration_ms &&
    p.beep.repeats === beep.repeats &&
    p.beep.volume === beep.volume;

  // The page area unfolds to its new height when the mode changes.
  $effect.pre(() => {
    void mode;
    before = page?.offsetHeight ?? 0;
  });
  $effect(() => {
    void mode;
    if (!page || !before || reduced()) return;
    const after = page.offsetHeight;
    if (Math.abs(after - before) > 1) {
      play(page, [{ height: `${before}px`, overflow: 'hidden' }, { height: `${after}px`, overflow: 'hidden' }], '--t-disclosure', '--ease-soft');
    }
  });
</script>

<div class="controls">
  <div class="top">
    <Segmented options={modes} value={mode} label="Sound type" onchange={(m) => onchange(m, beep, file)} />
    <Button small onclick={test} label="Test sound">
      <SpeakerIcon size={16} {playing} /> Test
    </Button>
  </div>

  <div class="page" bind:this={page}>
    {#key mode}
      <div class="inner" in:swapIn>
        {#if mode === 'universal'}
          <p class="note">{universalNote}</p>
        {:else if mode === 'beep'}
          <div class="presets" role="group" aria-label="Ready-made beeps">
            {#each BEEP_PRESETS as p}
              <button type="button" class="preset" class:on={isPreset(p)} aria-pressed={isPreset(p)} onclick={() => onchange('beep', { ...beep, ...p.beep }, file)}>
                {p.name}
              </button>
            {/each}
          </div>
          <div class="wave"><Waveform {beep} /></div>
          <div class="grid">
            <span class="lbl">Pitch</span>
            <Slider label="Pitch" value={beep.frequency_hz} min={300} max={2000} step={50} format={(v) => `${v} Hz`} onchange={(v) => set('frequency_hz', v)} />
            <span class="lbl">Length</span>
            <Slider label="Length" value={beep.duration_ms} min={50} max={600} step={10} format={(v) => `${v} ms`} onchange={(v) => set('duration_ms', v)} />
            <span class="lbl">Beeps</span>
            <Slider label="Beeps" value={beep.repeats} min={1} max={6} step={1} format={(v) => `${v}×`} onchange={(v) => set('repeats', v)} />
            <span class="lbl">Volume</span>
            <Slider label="Volume" value={beep.volume} min={0.1} max={1} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onchange={(v) => set('volume', v)} />
          </div>
        {:else}
          <div class="file">
            <span class="name" class:empty={!file}>{file ? baseName(file) : 'No file chosen'}</span>
            <Button small onclick={browse}>Choose file…</Button>
            <Button small kind="ghost" disabled={!file} onclick={() => onchange('file', beep, '')}>Remove</Button>
          </div>
          <p class="note">WAV, MP3, OGG or FLAC. The file is copied into VC Alert, so moving the original is fine.</p>
        {/if}
      </div>
    {/key}
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .controls {
    display: grid;
    gap: var(--sp-3);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }
  .inner {
    display: grid;
    gap: var(--sp-3);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }
  .preset {
    height: var(--h-sm);
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: var(--r-sm);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--text-2);
    font-size: var(--fs-sm);
    transition:
      background-color var(--t-fade) var(--ease-color),
      color var(--t-fade) var(--ease-color),
      transform var(--spring-press-dur) var(--spring-press-ease);
  }
  .preset:hover {
    color: var(--text);
    background: var(--hover);
  }
  .preset:active {
    transform: scale(var(--press-scale));
  }
  .preset.on {
    background: var(--accent-wash);
    box-shadow: inset 0 0 0 1px var(--accent);
    color: var(--accent-text);
  }
  .wave {
    display: flex;
  }
  .grid {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    align-items: center;
    column-gap: var(--sp-3);
    row-gap: var(--sp-1);
  }
  .lbl {
    color: var(--text-2);
  }
  .file {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .name {
    flex: 1;
    min-width: 0;
    height: var(--h-md);
    display: flex;
    align-items: center;
    padding: 0 var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--line);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .name.empty {
    color: var(--text-3);
  }
  .note {
    margin: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .error {
    margin: 0;
    color: var(--danger-text);
    font-size: var(--fs-sm);
  }
  @media (prefers-reduced-motion: reduce) {
    .preset:active {
      transform: none;
    }
  }
</style>
