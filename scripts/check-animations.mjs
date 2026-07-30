#!/usr/bin/env node
// Анимации не должны трогать раскладку: transition/animation на width,
// height, top и т.п. заставляют браузер пересчитывать layout каждый кадр —
// это и есть «дёргание». Проверяем СОБРАННЫЙ CSS (web/dist/assets), поэтому
// запускается после `pnpm build` (в CI — джоб web), а не в `make check`.
// Исключения — scripts/animation-allowlist.txt: `свойство — причина`.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const cssDir = process.argv[2] ?? join(root, "web", "dist", "assets");

// Анимировать безопасно только то, что не меняет раскладку.
const SAFE = new Set([
  "transform", "translate", "scale", "rotate", "opacity", "visibility",
  "color", "background-color", "border-color", "outline-color",
  "text-decoration-color", "caret-color", "fill", "stroke",
  "box-shadow", "filter", "backdrop-filter", "none",
]);

const allowlistPath = join(root, "scripts", "animation-allowlist.txt");
const allowed = new Set(
  (existsSync(allowlistPath) ? readFileSync(allowlistPath, "utf8") : "")
    .split("\n")
    .filter((l) => l.trim() && !l.startsWith("#"))
    .map((l) => l.split("—")[0].trim()),
);

const isSafe = (prop) =>
  prop.startsWith("--") || SAFE.has(prop) || allowed.has(prop);

// Вытащить тело @keyframes по балансу скобок (CSS минифицирован).
function keyframesBodies(css) {
  const bodies = [];
  const re = /@keyframes[^{]*\{/g;
  for (let m; (m = re.exec(css)); ) {
    let depth = 1;
    let i = re.lastIndex;
    while (i < css.length && depth > 0) {
      if (css[i] === "{") depth += 1;
      if (css[i] === "}") depth -= 1;
      i += 1;
    }
    bodies.push(css.slice(re.lastIndex, i - 1));
  }
  return bodies;
}

let failed = false;
const report = (file, where, prop) => {
  console.error(`АНИМАЦИЯ LAYOUT-СВОЙСТВА: ${file}: ${where}: ${prop}`);
  failed = true;
};

const files = existsSync(cssDir)
  ? readdirSync(cssDir).filter((f) => f.endsWith(".css"))
  : [];
if (files.length === 0) {
  console.error(`Нет собранного CSS в ${cssDir} — сначала pnpm build`);
  process.exit(1);
}

for (const file of files) {
  const css = readFileSync(join(cssDir, file), "utf8");

  for (const m of css.matchAll(/transition-property\s*:\s*([^;}]+)/g)) {
    for (const prop of m[1].split(",").map((p) => p.trim().toLowerCase())) {
      if (prop && !isSafe(prop)) report(file, "transition-property", prop);
    }
  }

  // Шорткат `transition: color .2s, width .3s` — свойство идёт первым словом.
  for (const m of css.matchAll(/[{;]\s*transition\s*:\s*([^;}]+)/g)) {
    for (const part of m[1].split(",")) {
      const prop = part.trim().split(/[\s(]/)[0].toLowerCase();
      if (prop && !/^[\d.]/.test(prop) && !isSafe(prop)) {
        report(file, "transition", prop);
      }
    }
  }

  for (const body of keyframesBodies(css)) {
    for (const m of body.matchAll(/([a-z-]+)\s*:/g)) {
      if (!isSafe(m[1])) report(file, "@keyframes", m[1]);
    }
  }
}

if (failed) {
  console.error(
    "Анимируй transform/opacity/цвета; исключение — в scripts/animation-allowlist.txt с причиной.",
  );
  process.exit(1);
}
console.log(`check-animations: ok (${files.length} css)`);
