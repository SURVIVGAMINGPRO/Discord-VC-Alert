// Motion helpers. Every duration and curve is read from styles/tokens.css at
// runtime, so the tokens stay the only source of motion values. Svelte's
// transition:/animate: directives need JS easing functions, so the CSS
// linear()/cubic-bezier() strings are turned into functions here.
//
// Rules (see the brief): only transform, opacity and filter move (height only
// for unfolding sections); under reduced motion things may fade but nothing
// moves, scales or shakes; anything fading out becomes inert.

import type { TransitionConfig } from 'svelte/transition';
import { flip, type AnimationConfig } from 'svelte/animate';

const cache = new Map<string, string>();

export function tok(name: string): string {
  let v = cache.get(name);
  if (v === undefined) {
    v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    cache.set(name, v);
  }
  return v;
}

export function ms(name: string): number {
  const v = tok(name);
  if (v.endsWith('ms')) return parseFloat(v);
  if (v.endsWith('s')) return parseFloat(v) * 1000;
  return parseFloat(v) || 0;
}

export function num(name: string): number {
  return parseFloat(tok(name)) || 0;
}

export const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

// ------------------------------------------------------------- easings --

function cubic(x1: number, y1: number, x2: number, y2: number) {
  const a = (a1: number, a2: number) => 1 - 3 * a2 + 3 * a1;
  const b = (a1: number, a2: number) => 3 * a2 - 6 * a1;
  const c = (a1: number) => 3 * a1;
  const at = (t: number, a1: number, a2: number) => ((a(a1, a2) * t + b(a1, a2)) * t + c(a1)) * t;
  const slope = (t: number, a1: number, a2: number) => 3 * a(a1, a2) * t * t + 2 * b(a1, a2) * t + c(a1);
  return (x: number) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) {
      const s = slope(t, x1, x2);
      if (Math.abs(s) < 1e-6) break;
      t -= (at(t, x1, x2) - x) / s;
    }
    return at(t, y1, y2);
  };
}

function linearPoints(points: number[]) {
  const n = points.length - 1;
  return (x: number) => {
    if (x <= 0) return points[0];
    if (x >= 1) return points[n];
    const p = x * n;
    const i = Math.floor(p);
    return points[i] + (points[i + 1] - points[i]) * (p - i);
  };
}

const fnCache = new Map<string, (t: number) => number>();

/** A JS easing function for a token like --ease-out or --spring-reflow-ease. */
export function ease(name: string): (t: number) => number {
  let f = fnCache.get(name);
  if (f) return f;
  const v = tok(name);
  const bez = v.match(/cubic-bezier\(([^)]+)\)/);
  const lin = v.match(/linear\(([^)]+)\)/);
  if (bez) {
    const [x1, y1, x2, y2] = bez[1].split(',').map(Number);
    f = cubic(x1, y1, x2, y2);
  } else if (lin) {
    f = linearPoints(lin[1].split(',').map((s) => parseFloat(s)));
  } else {
    f = cubic(0.23, 1, 0.32, 1);
  }
  fnCache.set(name, f);
  return f;
}

/** The CSS easing string for the Web Animations API, if this engine supports it. */
export function easeCss(name: string): string {
  const v = tok(name);
  return CSS.supports('transition-timing-function', v) ? v : 'ease-out';
}

/** Run a WAAPI animation from tokens; cancels the previous one on the same element. */
const running = new WeakMap<Element, Animation>();
export function play(
  el: Element,
  frames: Keyframe[],
  dur: string,
  curve: string,
  opts: KeyframeAnimationOptions = {},
): Animation | null {
  if (!el.animate) return null;
  running.get(el)?.cancel();
  const a = el.animate(frames, { duration: ms(dur), easing: easeCss(curve), ...opts });
  running.set(el, a);
  return a;
}

function makeInert(node: Element) {
  (node as HTMLElement).inert = true;
}

/** An entrance (including a reversed exit) must be interactive again. */
function alive(node: Element) {
  (node as HTMLElement).inert = false;
  (node as HTMLElement).style.pointerEvents = '';
}

// --------------------------------------------------------- transitions --

/** For helper layers (click catchers) that just need to stop catching at once. */
export function gone(node: Element): TransitionConfig {
  makeInert(node);
  (node as HTMLElement).style.pointerEvents = 'none';
  return { duration: 0 };
}

/** A row arriving: fades in and rises 8 px. */
export function rise(_node: Element, { y = 8, skip = false } = {}): TransitionConfig {
  alive(_node);
  if (skip) return { duration: 0 };
  const r = reduced();
  return {
    duration: r ? ms('--t-fade') : ms('--spring-reflow-dur'),
    easing: r ? ease('--ease-out') : ease('--spring-reflow-ease'),
    css: (t) => (r ? `opacity:${t}` : `opacity:${Math.min(1, t)};transform:translateY(${(1 - t) * y}px)`),
  };
}

/** A row leaving: fades out and shrinks to 0.96. */
export function shrink(node: Element, { skip = false } = {}): TransitionConfig {
  makeInert(node);
  if (skip) return { duration: 0 };
  const r = reduced();
  return {
    duration: ms('--t-fade'),
    easing: ease('--ease-out'),
    css: (t) => (r ? `opacity:${t}` : `opacity:${t};transform:scale(${0.96 + 0.04 * t})`),
  };
}

/** animate:flip parameters: rows glide to their new place. */
export function flipParams(skip = false) {
  return reduced() || skip
    ? { duration: 0 }
    : { duration: ms('--spring-reflow-dur'), easing: ease('--spring-reflow-ease') };
}

/**
 * animate:glide: flip, plus the row that travels furthest is drawn on top for
 * the length of the move, so a joiner rising past others stays visible.
 */
export function glide(
  node: Element,
  rects: { from: DOMRect; to: DOMRect },
  params: { duration?: number; easing?: (t: number) => number } = {},
): AnimationConfig {
  const cfg = flip(node, rects, params);
  const dy = Math.abs(rects.from.top - rects.to.top);
  const el = node as HTMLElement;
  if (dy > 2 && (cfg.duration ?? 0) > 0) {
    el.style.zIndex = String(2 + Math.round(dy));
    setTimeout(() => (el.style.zIndex = ''), cfg.duration);
  }
  return cfg;
}

/** Text swapping in place: clears from a light blur. */
export function swapIn(_node: Element): TransitionConfig {
  alive(_node);
  const blur = num('--blur-icon');
  return {
    duration: ms('--t-swap'),
    easing: ease('--ease-out'),
    css: (t) => `opacity:${t};filter:blur(${(1 - t) * blur}px)`,
  };
}

/** The outgoing label steps out of the layout so the new one sets the size. */
export function swapOut(node: Element): TransitionConfig {
  const el = node as HTMLElement;
  makeInert(el);
  el.style.position = 'absolute';
  el.style.inset = '0';
  el.style.display = 'flex';
  el.style.alignItems = 'center';
  el.style.justifyContent = 'center';
  const blur = num('--blur-icon');
  return {
    duration: ms('--t-swap'),
    easing: ease('--ease-out'),
    css: (t) => `opacity:${t};filter:blur(${(1 - t) * blur}px)`,
  };
}

/** Icons swapping in place: scale from 0.5 out of a blur. */
export function iconIn(_node: Element): TransitionConfig {
  alive(_node);
  const blur = num('--blur-icon');
  const r = reduced();
  return {
    duration: ms('--t-swap'),
    easing: ease('--ease-out'),
    css: (t) => `opacity:${t};filter:blur(${(1 - t) * blur}px)${r ? '' : `;transform:scale(${0.5 + 0.5 * t})`}`,
  };
}

export function iconOut(node: Element): TransitionConfig {
  const el = node as HTMLElement;
  makeInert(el);
  el.style.position = 'absolute';
  el.style.inset = '0';
  const cfg = iconIn(node);
  makeInert(el);
  return cfg;
}

/** Menus grow from the button or the pointer. Set transform-origin on the node. */
export function menuPop(node: Element, { out = false } = {}): TransitionConfig {
  if (out) makeInert(node);
  else alive(node);
  const from = num('--menu-from');
  const r = reduced();
  return {
    duration: ms('--t-menu'),
    easing: ease('--ease-soft'),
    // Opaque early (so nothing shows through), while the scale keeps easing.
    css: (t) => (r ? `opacity:${t}` : `opacity:${out ? t : Math.min(1, t * 2.5)};transform:scale(${from + (1 - from) * t})`),
  };
}

export function toastIn(_node: Element): TransitionConfig {
  alive(_node);
  const from = num('--toast-from');
  const blur = num('--blur-surface');
  const r = reduced();
  return {
    duration: r ? ms('--t-fade') : ms('--spring-toast-dur'),
    easing: r ? ease('--ease-out') : ease('--spring-toast-ease'),
    css: (t) =>
      r
        ? `opacity:${t}`
        : `opacity:${Math.min(1, t * 2)};filter:blur(${Math.max(0, 1 - t) * blur}px);transform:translateY(${(1 - t) * 12}px) scale(${from + (1 - from) * t})`,
  };
}

export function toastOut(node: Element): TransitionConfig {
  makeInert(node);
  const from = num('--toast-from');
  const r = reduced();
  return {
    duration: ms('--t-dismiss'),
    easing: ease('--ease-away'),
    css: (t) => (r ? `opacity:${t}` : `opacity:${t};transform:scale(${from + (1 - from) * t})`),
  };
}

export function dialogIn(_node: Element): TransitionConfig {
  alive(_node);
  const from = num('--dialog-from');
  const r = reduced();
  return {
    duration: r ? ms('--t-fade') : ms('--spring-dialog-dur'),
    easing: r ? ease('--ease-out') : ease('--spring-dialog-ease'),
    css: (t) => (r ? `opacity:${t}` : `opacity:${Math.min(1, t * 2.5)};transform:scale(${from + (1 - from) * t})`),
  };
}

export function dialogOut(node: Element): TransitionConfig {
  makeInert(node);
  const r = reduced();
  return {
    duration: ms('--t-dismiss'),
    easing: ease('--ease-away'),
    css: (t) => (r ? `opacity:${t}` : `opacity:${t};transform:scale(${0.96 + 0.04 * t})`),
  };
}

export function backdropIn(_node: Element): TransitionConfig {
  alive(_node);
  return { duration: ms('--t-overlay'), easing: ease('--ease-soft'), css: (t) => `opacity:${t}` };
}

export function backdropOut(node: Element): TransitionConfig {
  makeInert(node);
  return { duration: ms('--t-dismiss'), easing: ease('--ease-away'), css: (t) => `opacity:${t}` };
}

/** Unfolding sections and banners: height opens, content clears from a blur. */
export function unfold(node: Element): TransitionConfig {
  alive(node);
  const el = node as HTMLElement;
  const h = el.scrollHeight;
  const blur = num('--blur-icon');
  return {
    duration: ms('--t-disclosure'),
    easing: ease('--ease-soft'),
    css: (t) => `overflow:hidden;height:${t * h}px;opacity:${t};filter:blur(${(1 - t) * blur}px)`,
  };
}

export function fold(node: Element): TransitionConfig {
  const cfg = unfold(node);
  makeInert(node);
  return cfg;
}

/** Wizard steps: slide 24 px sideways out of a light blur, in the travel direction. */
export function step(node: Element, { dir = 1, out = false } = {}): TransitionConfig {
  if (out) {
    makeInert(node);
    const el = node as HTMLElement;
    el.style.position = 'absolute';
    el.style.inset = '0';
  }
  const blur = num('--blur-icon');
  const r = reduced();
  const sign = out ? -dir : dir;
  // A hand-off, not a crossfade: the old step is gone by 40% and the new one
  // only shows after that, so their text never overlaps.
  const visible = (t: number) => (out ? Math.max(0, (t - 0.6) / 0.4) : Math.max(0, (t - 0.4) / 0.6));
  return {
    duration: ms('--t-disclosure'),
    easing: ease('--ease-out'),
    css: (t) =>
      `opacity:${visible(t)};filter:blur(${(1 - t) * blur}px)${r ? '' : `;transform:translateX(${(1 - t) * 24 * sign}px)`}`,
  };
}

/** One digit of a rolling number: slides a full line up or down. */
export function digit(node: Element, { dir = 1, out = false } = {}): TransitionConfig {
  if (out) {
    const el = node as HTMLElement;
    el.style.position = 'absolute';
    el.style.left = '0';
    el.style.top = '0';
  }
  const r = reduced();
  const sign = out ? -dir : dir;
  return {
    duration: r ? ms('--t-fade') : ms('--t-roll'),
    easing: ease('--ease-out'),
    css: (t) => (r ? `opacity:${t}` : `transform:translateY(${(1 - t) * 100 * sign}%);opacity:${t}`),
  };
}

// ------------------------------------------------------- one-off motion --

/** Invalid input: 3 px left/right swings (3, 2, 1). */
export function shake(el: Element | undefined | null) {
  if (!el || reduced()) return;
  const d = num('--shake');
  play(
    el,
    [0, -d, d, -d * 0.66, d * 0.66, -d * 0.33, d * 0.33, 0].map((x) => ({ transform: `translateX(${x}px)` })),
    '--t-shake',
    '--ease-out',
  );
}

/** The overlay count pill: one small bump when the count goes up. */
export function bump(el: Element | undefined | null) {
  if (!el || reduced()) return;
  play(el, [{ transform: 'scale(1.15)' }, { transform: 'scale(1)' }], '--spring-bounce-dur', '--spring-bounce-ease');
}

/**
 * Slide a shared highlight to the selected item (FLIP): set the new box at
 * once, then animate the transform from where it is right now, so a second
 * click mid-flight retargets smoothly.
 */
export function slideHighlight(hl: HTMLElement | undefined, target: HTMLElement | undefined | null, animate = true) {
  if (!hl || !target) return;
  const before = hl.getBoundingClientRect();
  const parent = hl.offsetParent as HTMLElement | null;
  hl.style.left = `${target.offsetLeft}px`;
  hl.style.top = `${target.offsetTop}px`;
  hl.style.width = `${target.offsetWidth}px`;
  hl.style.height = `${target.offsetHeight}px`;
  hl.style.opacity = '1';
  if (!animate || !parent || before.width === 0 || reduced()) return;
  const after = hl.getBoundingClientRect();
  if (after.width === 0) return;
  const dx = before.left - after.left;
  const dy = before.top - after.top;
  const sx = before.width / after.width;
  const sy = before.height / after.height;
  if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5 && Math.abs(sx - 1) < 0.01 && Math.abs(sy - 1) < 0.01) return;
  play(
    hl,
    [{ transform: `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})` }, { transform: 'none' }],
    '--spring-selection-dur',
    '--spring-selection-ease',
  );
}
