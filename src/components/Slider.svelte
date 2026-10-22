<script lang="ts">
  // Fires onchange at most every 70 ms while dragging, and once more on release (v1).
  let {
    value,
    min,
    max,
    step,
    label,
    format = (v: number) => String(v),
    onchange,
  }: {
    value: number;
    min: number;
    max: number;
    step: number;
    label: string;
    format?: (v: number) => string;
    onchange: (v: number) => void;
  } = $props();

  let local = $state(0);
  let dragging = $state(false);
  let last = 0;
  let pending: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!dragging) local = value;
  });

  const snap = (v: number) => {
    const s = min + Math.round((v - min) / step) * step;
    const c = Math.min(max, Math.max(min, s));
    return Number.isInteger(step) && Number.isInteger(min) ? Math.round(c) : Math.round(c * 100) / 100;
  };

  function fire(now = false) {
    clearTimeout(pending);
    const t = Date.now();
    if (now || t - last >= 70) {
      last = t;
      onchange(local);
    } else {
      pending = setTimeout(() => fire(true), 70 - (t - last));
    }
  }

  const pct = $derived(((local - min) / (max - min)) * 100);
</script>

<div class="slider">
  <input
    type="range"
    {min}
    {max}
    {step}
    aria-label={label}
    aria-valuetext={format(local)}
    value={local}
    style:--pct="{pct}%"
    oninput={(e) => {
      dragging = true;
      local = snap(Number((e.target as HTMLInputElement).value));
      fire();
    }}
    onchange={() => {
      dragging = false;
      fire(true);
    }}
  />
  <span class="val num">{format(local)}</span>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
  }
  input {
    flex: 1;
    min-width: 0;
    height: var(--h-sm);
    margin: 0;
    background: transparent;
    appearance: none;
    -webkit-appearance: none;
  }
  input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: var(--r-full);
    background: linear-gradient(to right, var(--accent) var(--pct), var(--line-strong) var(--pct));
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border-radius: var(--r-full);
    background: var(--on-accent);
    box-shadow:
      0 0 0 1px var(--line-strong),
      var(--shadow-pop);
  }
  input:focus-visible {
    outline: none;
  }
  input:focus-visible::-webkit-slider-thumb {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .val {
    min-width: 7ch;
    text-align: right;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
</style>
