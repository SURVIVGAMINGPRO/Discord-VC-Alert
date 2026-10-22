// Renders the app icon and tray icons from assets/appicon.svg.
// Run `npm run icons`, then `npx tauri icon src-tauri/icons/app-icon.png`.
import { Resvg } from '@resvg/resvg-js';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';

const svg = readFileSync('assets/appicon.svg', 'utf8');
const path = svg.match(/ d="([^"]+)"/)[1];
const VB = 'viewBox="0 -960 960 960"';

function png(markup, size) {
  return new Resvg(markup, { fitTo: { mode: 'width', value: size } }).render().asPng();
}

function glyph({ color, opacity = 1, slash = false }) {
  const cut = slash
    ? `<mask id="m"><rect x="0" y="-960" width="960" height="960" fill="white"/>
         <line x1="110" y1="-880" x2="870" y2="-80" stroke="black" stroke-width="190" stroke-linecap="round"/></mask>`
    : '';
  const line = slash
    ? `<line x1="110" y1="-880" x2="870" y2="-80" stroke="${color}" stroke-width="80" stroke-linecap="round"/>`
    : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" ${VB} width="960" height="960">
    <defs>${cut}</defs>
    <g opacity="${opacity}">
      <path d="${path}" fill="${color}" ${slash ? 'mask="url(#m)"' : ''}/>
      ${line}
    </g></svg>`;
}

mkdirSync('src-tauri/icons/tray', { recursive: true });
writeFileSync('src-tauri/icons/app-icon.png', png(svg.replace(/width="48px"/, 'width="960"').replace(/height="48px"/, 'height="960"'), 1024));

// macOS menu bar: template images (alpha only), 22 pt @2x.
writeFileSync('src-tauri/icons/tray/mac-idle.png', png(glyph({ color: '#000', opacity: 0.55 }), 44));
writeFileSync('src-tauri/icons/tray/mac-watching.png', png(glyph({ color: '#000' }), 44));
writeFileSync('src-tauri/icons/tray/mac-quiet.png', png(glyph({ color: '#000', slash: true }), 44));
// Windows notification area: coloured, 32 px.
writeFileSync('src-tauri/icons/tray/win-idle.png', png(glyph({ color: '#949BA4' }), 32));
writeFileSync('src-tauri/icons/tray/win-watching.png', png(glyph({ color: '#5865F2' }), 32));
writeFileSync('src-tauri/icons/tray/win-quiet.png', png(glyph({ color: '#949BA4', slash: true }), 32));
console.log('icons written');
