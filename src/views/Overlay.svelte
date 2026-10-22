<script lang="ts">
  // The live overlay. Rust sizes and places the (click-through) window and
  // tells this page what to draw and which picture the cursor is over. The
  // only motion: a one-shot join halo, the count roll and bump, and the hover
  // bubble growing and gliding. Nothing loops.
  import { onMount, tick } from 'svelte';
  import RollingNumber from '../components/RollingNumber.svelte';
  import { call, inTauri, on } from '../lib/api';
  import { bump, num, play, reduced, swapIn } from '../lib/motion';
  import { avatarColor, initialOf } from '../lib/util';
  import type { OverlayCell, OverlayModel } from '../lib/types';

  let model = $state<OverlayModel | null>(null);
  let hover = $state<number | null>(null);
  let halos: Record<string, HTMLSpanElement> = $state({});
  let pill: HTMLDivElement | undefined = $state();
  let lastFlash: string | null = null;
  let lastCount = 0;

  async function receive(m: OverlayModel) {
    const first = model === null;
    const prevCount = lastCount;
    model = m;
    lastCount = m.count;
    await tick();
    if (!first && m.style === 'count' && m.count > prevCount) bump(pill);
    if (m.flash && m.flash !== lastFlash) halo(m.flash);
    lastFlash = m.flash;
  }

  function halo(key: string) {
    const el = halos[key];
    if (!el) return;
    if (reduced()) {
      play(el, [{ opacity: 1 }, { opacity: 1, offset: 0.9 }, { opacity: 0 }], '--t-flash', '--ease-out');
    } else {
      // Expands once and stays visible for most of the move, then fades.
      play(
        el,
        [
          { opacity: 1, transform: 'scale(1)' },
          { opacity: 0.9, transform: 'scale(1.45)', offset: 0.55 },
          { opacity: 0, transform: 'scale(1.8)' },
        ],
        '--t-ping',
        '--ease-soft',
      );
    }
  }

  onMount(() => {
    document.documentElement.dataset.view = 'overlay';
    if (!inTauri) {
      import('../lib/mock').then(({ previewBackdrop }) => {
        document.body.style.background = previewBackdrop(new URLSearchParams(location.search).get('bg'));
      });
    }
    const offs = [on<OverlayModel>('overlay', receive), on<number | null>('overlay-hover', (i) => (hover = i))];
    call('overlay_refresh');
    return () => offs.forEach((p) => p.then((f) => f()));
  });

  // In a plain browser (UI preview) there is no Rust hit-testing, so do it here.
  function previewHover(e: PointerEvent) {
    if (inTauri || !model) return;
    if (model.style === 'count') {
      const { bar_x, bar_y } = model.canvas;
      const inside = e.clientX >= bar_x && e.clientX <= bar_x + model.bar_w && e.clientY >= bar_y && e.clientY <= bar_y + model.bar_h;
      hover = inside ? 0 : null;
      return;
    }
    const r = model.slot / 2 + 2;
    const i = model.cells.findIndex((c) => Math.hypot(e.clientX - (c.x + model!.slot / 2), e.clientY - (c.y + model!.slot / 2)) <= r);
    hover = i >= 0 ? i : null;
  }

  // ------------------------------------------------------------ bubble --
  const bubbleText = $derived.by<[string, string] | null>(() => {
    if (!model || hover === null) return null;
    if (model.style === 'count') return model.count ? model.count_bubble : null;
    const c = model.cells[hover];
    if (!c || c.item.kind === 'empty') return null;
    return c.bubble;
  });

  let bw = $state(0);
  let bh = $state(0);
  const GAP = 6;

  const anchor = $derived.by(() => {
    if (!model || hover === null) return null;
    if (model.style === 'count') {
      const { bar_x, bar_y } = model.canvas;
      return { cx: bar_x + model.bar_w / 2, cy: bar_y + model.bar_h / 2, r: model.bar_h / 2, rx: model.bar_w / 2 };
    }
    const c = model.cells[hover];
    if (!c) return null;
    return { cx: c.x + model.slot / 2, cy: c.y + model.slot / 2, r: model.slot / 2, rx: model.slot / 2 };
  });

  const place = $derived.by(() => {
    if (!model || !anchor) return null;
    const W = model.canvas.width;
    const H = model.canvas.height;
    const side = model.side;
    let x: number;
    let y: number;
    if (side === 'below' || side === 'above') {
      x = Math.min(Math.max(4, anchor.cx - bw / 2), W - bw - 4);
      y = side === 'below' ? anchor.cy + anchor.r + GAP : anchor.cy - anchor.r - GAP - bh;
    } else {
      y = Math.min(Math.max(4, anchor.cy - bh / 2), H - bh - 4);
      x = side === 'right' ? anchor.cx + anchor.rx + GAP : anchor.cx - anchor.rx - GAP - bw;
    }
    const px = Math.min(Math.max(10, anchor.cx - x), bw - 10);
    const py = Math.min(Math.max(10, anchor.cy - y), bh - 10);
    return { x, y, px, py, side };
  });

  let shown = $state(false);
  let gliding = $state(false);
  let inner: HTMLDivElement | undefined = $state();
  $effect(() => {
    const visible = !!bubbleText && !!place;
    if (visible && !shown) {
      gliding = false;
      shown = true;
      tick().then(() => {
        const from = num('--tooltip-from');
        if (reduced()) play(inner!, [{ opacity: 0 }, { opacity: 1 }], '--t-fade', '--ease-out');
        else play(inner!, [{ opacity: 0, transform: `scale(${from})` }, { opacity: 1, transform: 'scale(1)' }], '--spring-pop-dur', '--spring-pop-ease');
        // Glide only once the first position (and the measured size) is in place.
        requestAnimationFrame(() => requestAnimationFrame(() => (gliding = true)));
      });
    } else if (!visible && shown) {
      shown = false;
      gliding = false;
    }
  });

  const origin = $derived(
    place
      ? place.side === 'below'
        ? `${place.px}px 0`
        : place.side === 'above'
          ? `${place.px}px 100%`
          : place.side === 'right'
            ? `0 ${place.py}px`
            : `100% ${place.py}px`
      : '50% 50%',
  );

  const personCell = (c: OverlayCell) => (c.item.kind === 'person' ? c.item : null);
</script>

<svelte:window onpointermove={previewHover} onpointerleave={() => !inTauri && (hover = null)} />

{#if model}
  <div class="canvas" style:width="{model.canvas.width}px" style:height="{model.canvas.height}px" style:opacity={model.opacity}>
    {#if model.style === 'count'}
      <div
        class="pill num"
        bind:this={pill}
        style:left="{model.canvas.bar_x + 2}px"
        style:top="{model.canvas.bar_y + 2}px"
        style:width="{model.bar_w - 4}px"
        style:height="{model.bar_h - 4}px"
        style:font-size="{Math.max(11, Math.round((model.bar_h - 4) * 0.55))}px"
      >
        <span class="pill-dot" class:none={!model.count}></span>
        <RollingNumber value={model.count} />
      </div>
    {:else}
      {#each model.cells as c, i (c.item.kind === 'person' ? c.item.key : `${c.item.kind}-${i}`)}
        {@const p = personCell(c)}
        <div
          class="slot"
          class:dots={model.style === 'dots'}
          style:left="{c.x}px"
          style:top="{c.y}px"
          style:width="{model.slot}px"
          style:height="{model.slot}px"
          style:--px="{model.px}px"
        >
          {#if p}
            {#if c.avatar && model.style === 'pictures'}
              <img class="face" src={c.avatar} alt="" draggable="false" />
            {:else}
              <span class="face letter" style:background={avatarColor(p.key)} style:font-size="{Math.max(8, Math.round(model.px * 0.44))}px">
                {#if model.style === 'pictures' && model.px >= 14}{initialOf(p.label)}{/if}
              </span>
            {/if}
            <!-- After the face, so the green ring sits on top of the dark one. -->
            <span class="halo" bind:this={halos[p.key]}></span>
          {:else if c.item.kind === 'badge'}
            <span class="face badge num" style:font-size="{Math.max(8, Math.round(model.px * 0.38))}px">{model.style === 'dots' ? '' : `+${c.item.count}`}</span>
          {:else}
            <span class="face empty"></span>
          {/if}
        </div>
      {/each}
    {/if}

    {#if shown && place && bubbleText}
      <div
        class="bubble-pos"
        class:glide={gliding}
        style:transform="translate({Math.round(place.x)}px, {Math.round(place.y)}px)"
        bind:clientWidth={bw}
        bind:clientHeight={bh}
      >
        <div class="bubble {place.side}" bind:this={inner} style:transform-origin={origin} style:--px="{place.px}px" style:--py="{place.py}px">
          {#key bubbleText[0] + bubbleText[1]}
            <div class="text" in:swapIn>
              <div class="main">{bubbleText[0]}</div>
              {#if bubbleText[1]}<div class="sub id">{bubbleText[1]}</div>{/if}
            </div>
          {/key}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .canvas {
    position: relative;
    overflow: visible;
    font-family: var(--font-ui);
    color: var(--ov-text);
  }
  .slot {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .face,
  .halo {
    width: var(--px);
    height: var(--px);
    border-radius: var(--r-full);
  }
  .face {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    object-fit: cover;
    box-shadow:
      0 0 0 2px var(--ov-ring),
      0 0 0 3px var(--ov-edge);
  }
  .dots .face {
    box-shadow:
      0 0 0 1.5px var(--ov-ring),
      0 0 0 2.5px var(--ov-edge);
  }
  .letter {
    color: var(--ov-text);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
  .badge {
    background: var(--ov-badge);
    color: var(--ov-text);
    font-weight: var(--fw-bold);
  }
  .empty {
    background: none;
    box-shadow: none;
    outline: 1.5px dashed var(--ov-edge);
    outline-offset: -1.5px;
  }
  .halo {
    position: absolute;
    left: 50%;
    top: 50%;
    margin: calc(var(--px) / -2) 0 0 calc(var(--px) / -2);
    box-shadow:
      0 0 0 2px var(--ov-live),
      0 0 0 3px var(--ov-ring);
    opacity: 0;
    pointer-events: none;
  }

  .pill {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4em;
    border-radius: var(--r-full);
    background: var(--ov-plate);
    box-shadow: 0 0 0 1px var(--ov-edge);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
  .pill-dot {
    width: 0.55em;
    height: 0.55em;
    border-radius: var(--r-full);
    background: var(--ov-live);
  }
  .pill-dot.none {
    background: var(--ov-text-2);
  }

  .bubble-pos {
    position: absolute;
    left: 0;
    top: 0;
    pointer-events: none;
  }
  .bubble-pos.glide {
    transition: transform var(--spring-pop-dur) var(--spring-pop-ease);
  }
  .bubble {
    position: relative;
    padding: var(--sp-1) var(--sp-2);
    border-radius: var(--r-sm);
    background: var(--ov-plate);
    box-shadow: 0 0 0 1px var(--ov-edge);
    white-space: nowrap;
  }
  .bubble::before {
    content: '';
    position: absolute;
    width: 8px;
    height: 8px;
    background: var(--ov-plate);
    transform: rotate(45deg);
  }
  .below::before {
    top: -4px;
    left: calc(var(--px) - 4px);
    box-shadow: -1px -1px 0 0 var(--ov-edge);
  }
  .above::before {
    bottom: -4px;
    left: calc(var(--px) - 4px);
    box-shadow: 1px 1px 0 0 var(--ov-edge);
  }
  .right::before {
    left: -4px;
    top: calc(var(--py) - 4px);
    box-shadow: -1px 1px 0 0 var(--ov-edge);
  }
  .left::before {
    right: -4px;
    top: calc(var(--py) - 4px);
    box-shadow: 1px -1px 0 0 var(--ov-edge);
  }
  .text {
    position: relative;
  }
  .main {
    font-size: var(--fs-sm);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  .sub {
    color: var(--ov-text-2);
    font-size: var(--fs-xs);
  }
  @media (prefers-reduced-motion: reduce) {
    .bubble-pos.glide {
      transition: none;
    }
  }
</style>
