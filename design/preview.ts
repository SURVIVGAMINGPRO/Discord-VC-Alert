// Design-direction preview: real tokens, real fonts, real icons.
import '@fontsource-variable/atkinson-hyperlegible-mono';
import '../src/styles/tokens.css';
import { ICONS } from '../src/lib/icons';

document.querySelectorAll<HTMLElement>('[data-icon]').forEach((el) => {
  el.classList.add('ico');
  el.innerHTML = ICONS[el.dataset.icon as keyof typeof ICONS] ?? '';
});

const NAMES = ['--bg', '--surface', '--raised', '--line-strong', '--text', '--text-2', '--text-3', '--accent', '--accent-text', '--live', '--warn', '--danger'];
for (const theme of ['ink', 'graphite', 'paper']) {
  const host = document.getElementById(`sw-${theme}`)!;
  const probe = host.closest('[data-theme]') as HTMLElement;
  const cs = getComputedStyle(probe);
  host.innerHTML =
    `<div style="grid-column:1/-1;background:none;box-shadow:none;height:auto;color:var(--text);font-weight:600">${theme[0].toUpperCase() + theme.slice(1)}</div>` +
    NAMES.map((n) => {
      const v = cs.getPropertyValue(n).trim();
      return `<div style="background:${v}"><span style="position:relative;top:36px">${n.slice(2)}</span></div>`;
    }).join('');
  host.style.rowGap = '22px';
}
