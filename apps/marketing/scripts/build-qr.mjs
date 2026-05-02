#!/usr/bin/env node
// Generate real QR-code SVGs for the demo end-card.
//
// Usage: npm run build:qr  (or)  node scripts/build-qr.mjs
//
// Writes:
//   public/demo-assets/qr-github.svg
//   public/demo-assets/qr-npm.svg
//   public/demo-assets/qr-cratesio.svg
//
// The hand-authored end-card.svg ships with a stylized QR-ish motif
// (so the asset works offline, no script run needed). After running
// this script, Daniel can splice the real-QR SVGs into the recording
// timeline in place of the stylized placeholders, or composite them
// over end-card.svg in the editor.

import { promises as fs } from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const here = path.dirname(url.fileURLToPath(import.meta.url));
const outDir = path.resolve(here, '..', 'public', 'demo-assets');

const targets = [
  {
    name: 'github',
    url: 'https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026',
    label: 'GITHUB',
  },
  {
    name: 'npm',
    url: 'https://www.npmjs.com/org/sbo3l',
    label: 'NPM',
  },
  {
    name: 'cratesio',
    url: 'https://crates.io/crates/sbo3l-cli',
    label: 'CRATES.IO',
  },
];

let QRCode;
try {
  QRCode = (await import('qrcode')).default;
} catch (err) {
  console.error(
    'build-qr: missing dependency "qrcode". Install it with:\n' +
      '  cd apps/marketing && npm install --save-dev qrcode\n' +
      'Then re-run this script.'
  );
  process.exitCode = 1;
  throw err;
}

await fs.mkdir(outDir, { recursive: true });

for (const t of targets) {
  const svg = await QRCode.toString(t.url, {
    type: 'svg',
    errorCorrectionLevel: 'M',
    margin: 1,
    color: { dark: '#000000', light: '#ffffff' },
  });
  const out = path.join(outDir, `qr-${t.name}.svg`);
  await fs.writeFile(out, svg, 'utf8');
  console.log(`build-qr: wrote ${path.relative(process.cwd(), out)}  →  ${t.url}`);
}

console.log(
  `\nbuild-qr: ${targets.length} QR SVGs ready in ${path.relative(process.cwd(), outDir)}`
);
