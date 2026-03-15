#!/usr/bin/env bun
// Generates all required app icons from an SVG template.
// Run with: bun run scripts/generate-icons.ts

import sharp from "sharp";
import { mkdirSync } from "fs";
import { join } from "path";

const ROOT = join(import.meta.dirname, "..");
const DESKTOP_ICONS = join(ROOT, "apps/desktop/src-tauri/icons");
const MOBILE_ASSETS = join(ROOT, "apps/mobile/assets");

mkdirSync(DESKTOP_ICONS, { recursive: true });
mkdirSync(MOBILE_ASSETS, { recursive: true });

function appIconSvg(size: number): string {
  const pad = Math.round(size * 0.078);    // ~80/1024
  const gap = Math.round(size * 0.063);    // ~64/1024
  const radius = Math.round(size * 0.195); // outer corner radius
  const cellW = Math.round((size - 2 * pad - gap) / 2);
  const cellR = Math.round(size * 0.039);  // cell corner radius

  const cells = [
    { x: pad, y: pad, letter: "B" },
    { x: pad + cellW + gap, y: pad, letter: "O" },
    { x: pad, y: pad + cellW + gap, letter: "R" },
    { x: pad + cellW + gap, y: pad + cellW + gap, letter: "G" },
  ];

  const fontSize = Math.round(cellW * 0.65);
  const rects = cells
    .map(
      (c) =>
        `<rect x="${c.x}" y="${c.y}" width="${cellW}" height="${cellW}" rx="${cellR}" fill="#92400e"/>` +
        `<text x="${c.x + cellW / 2}" y="${c.y + cellW * 0.68}" text-anchor="middle" font-family="'SF Pro Display','Helvetica Neue',Arial,sans-serif" font-weight="800" font-size="${fontSize}" fill="#f59e0b">${c.letter}</text>`
    )
    .join("\n  ");

  return `<svg width="${size}" height="${size}" xmlns="http://www.w3.org/2000/svg">
  <rect width="${size}" height="${size}" rx="${radius}" fill="#0f0e0c"/>
  ${rects}
</svg>`;
}

function splashSvg(w: number, h: number): string {
  const logoSize = Math.round(w * 0.35);
  const logoX = Math.round((w - logoSize) / 2);
  const logoY = Math.round(h * 0.32);
  const textY = logoY + logoSize + Math.round(h * 0.06);
  const textSize = Math.round(w * 0.11);

  const pad = Math.round(logoSize * 0.078);
  const gap = Math.round(logoSize * 0.063);
  const cellW = Math.round((logoSize - 2 * pad - gap) / 2);
  const cellR = Math.round(logoSize * 0.039);
  const fontSize = Math.round(cellW * 0.65);

  const cells = [
    { x: logoX + pad, y: logoY + pad, letter: "B" },
    { x: logoX + pad + cellW + gap, y: logoY + pad, letter: "O" },
    { x: logoX + pad, y: logoY + pad + cellW + gap, letter: "R" },
    { x: logoX + pad + cellW + gap, y: logoY + pad + cellW + gap, letter: "G" },
  ];

  const logoRadius = Math.round(logoSize * 0.195);
  const rects = cells
    .map(
      (c) =>
        `<rect x="${c.x}" y="${c.y}" width="${cellW}" height="${cellW}" rx="${cellR}" fill="#92400e"/>` +
        `<text x="${c.x + cellW / 2}" y="${c.y + cellW * 0.68}" text-anchor="middle" font-family="'SF Pro Display','Helvetica Neue',Arial,sans-serif" font-weight="800" font-size="${fontSize}" fill="#f59e0b">${c.letter}</text>`
    )
    .join("\n  ");

  return `<svg width="${w}" height="${h}" xmlns="http://www.w3.org/2000/svg">
  <rect width="${w}" height="${h}" fill="#0f0e0c"/>
  <rect x="${logoX}" y="${logoY}" width="${logoSize}" height="${logoSize}" rx="${logoRadius}" fill="#0f0e0c"/>
  ${rects}
  <text x="${w / 2}" y="${textY}" text-anchor="middle" font-family="'SF Pro Display','Helvetica Neue',Arial,sans-serif" font-weight="700" font-size="${textSize}" fill="#ffffff">Borg</text>
</svg>`;
}

async function generateIcon(svg: string, size: number, outPath: string) {
  await sharp(Buffer.from(svg)).resize(size, size).png().toFile(outPath);
  console.log(`  ${outPath.replace(ROOT + "/", "")} (${size}x${size})`);
}

async function main() {
  console.log("Generating app icons...\n");

  const baseSvg = appIconSvg(1024);

  // Desktop icons
  const desktopTargets: [string, number][] = [
    ["32x32.png", 32],
    ["128x128.png", 128],
    ["128x128@2x.png", 256],
    ["icon.png", 512],
  ];

  for (const [name, size] of desktopTargets) {
    await generateIcon(baseSvg, size, join(DESKTOP_ICONS, name));
  }

  // Mobile icons
  await generateIcon(baseSvg, 1024, join(MOBILE_ASSETS, "icon.png"));
  await generateIcon(baseSvg, 1024, join(MOBILE_ASSETS, "adaptive-icon.png"));

  // Splash screen
  const splash = splashSvg(1284, 2778);
  await sharp(Buffer.from(splash)).png().toFile(join(MOBILE_ASSETS, "splash.png"));
  console.log(`  apps/mobile/assets/splash.png (1284x2778)`);

  // .ico for Windows (multi-size ICO from 256px PNG)
  const ico256 = await sharp(Buffer.from(baseSvg)).resize(256, 256).png().toBuffer();
  await sharp(ico256).toFile(join(DESKTOP_ICONS, "icon.ico"));
  console.log(`  apps/desktop/src-tauri/icons/icon.ico (256x256)`);

  // .icns placeholder: Tauri build converts PNGs, but we provide a 512px PNG renamed
  const icns512 = await sharp(Buffer.from(baseSvg)).resize(512, 512).png().toBuffer();
  await sharp(icns512).toFile(join(DESKTOP_ICONS, "icon.icns"));
  console.log(`  apps/desktop/src-tauri/icons/icon.icns (512x512 PNG)`);

  console.log("\nDone.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
