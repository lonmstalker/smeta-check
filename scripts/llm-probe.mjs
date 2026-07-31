#!/usr/bin/env node
// Замер качества vision-моделей на фото смет — гейт волны LLM (F0 плана
// docs/plans/2026-07-llm-integration.md). Он же зовётся при смене модели
// или провайдера (см. runbook docs/llm.md): планка одна и та же.
//
// Корпус — локальный, в git не идёт (чужие сметы, см. docs/llm-data.md):
// `fixtures-local/photos/<класс>/<имя>.jpg` рядом с `<имя>.truth.txt` (что
// на кадре написано). Каталог = класс, планку каждый класс проходит сам.
// Ключи — в `.env.probe` в корне (тоже вне git).
// Запуск: node scripts/llm-probe.mjs [--dir …] [--models id,id] [--limit N]
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Кандидаты замера. Меняешь модель — правишь этот список и гоняешь заново.
const CANDIDATES = [
  { id: "gemini-3.1-flash-lite", model: "google/gemini-3.1-flash-lite" },
  { id: "gemini-3-flash", model: "google/gemini-3-flash-preview" },
  { id: "qwen3-vl-235b", model: "qwen/qwen3-vl-235b-a22b-instruct" },
  { id: "gpt-5-mini", model: "openai/gpt-5-mini" },
  {
    id: "yandex-gemma-3-27b",
    base: "https://llm.api.cloud.yandex.net/v1",
    model: "gpt://{YANDEX_FOLDER_ID}/gemma-3-27b-it",
    keyEnv: "YANDEX_API_KEY",
    scheme: "Api-Key",
  },
];
const OPENROUTER = "https://openrouter.ai/api/v1";

// Тот же промпт пойдёт в разбор фото (решение 12 плана): переписать, а не
// вычислить; неразборчивое — отдельным списком, а не выдуманным числом.
const PROMPT = `Ты переписываешь смету на ремонт с фотографии в JSON.

Верни ТОЛЬКО JSON без пояснений и без markdown:
{"lines":[{"name":"…","unit":"…","quantity":0,"price":0,"total":0}],"unreadable":["…"]}

Правила:
- Переписывай строки как видишь. Не восстанавливай и не вычисляй числа,
  которых нет на листе: нет числа — не ставь поле.
- Ничего не придумывай: каждое число должно быть видно на фотографии.
- Заголовки разделов («Стены», «Потолок») тоже строки — только с именем.
- Неразборчивое верни в "unreadable" как есть, а не догадкой.`;

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i === -1 ? fallback : args[i + 1];
};
const corpusDir = flag("dir", join(root, "fixtures-local", "photos"));
const only = flag("models", "").split(",").filter(Boolean);
const limit = Number(flag("limit", "0")) || Infinity;
const maxTokens = Number(flag("max-tokens", "4000"));

// .env.probe: KEY=VALUE, комментарии с решётки.
function readEnvFile(file) {
  const env = {};
  if (!existsSync(file)) return env;
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z_]+)\s*=\s*(.*?)\s*$/);
    if (m && !line.trimStart().startsWith("#")) env[m[1]] = m[2].replace(/^["']|["']$/g, "");
  }
  return env;
}
const env = { ...readEnvFile(join(root, ".env.probe")), ...process.env };

// --- разбор текста: числа и строки-позиции ------------------------------

// «1 234,56» и «1234.5» — одно и то же число; пробелы разделяют разряды.
function toNumber(raw) {
  const cleaned = raw.replace(/[\s ]/g, "").replace(",", ".");
  const value = Number(cleaned);
  return Number.isFinite(value) ? value : null;
}
// Пробел разделяет разряды только по три цифры: «1 611,01» — одно число,
// а «м2   17,98» — два (иначе соседние колонки склеиваются в 217,98).
const NUMBER = /\d{1,3}(?:[  ]\d{3})+(?:[.,]\d+)?|\d+(?:[.,]\d+)?/g;
const numbersIn = (text) =>
  [...text.matchAll(NUMBER)].map((m) => toNumber(m[0])).filter((n) => n !== null);

const near = (a, b) => Math.abs(a - b) <= Math.max(0.01, Math.abs(b) * 0.005);
const hasNumber = (pool, value) => pool.some((n) => near(value, n));

// «Плёнка» и «пленка» — одно слово: модели пишут ё как придётся.
const words = (text) =>
  new Set(
    text
      .toLowerCase()
      .replace(/ё/g, "е")
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .split(" ")
      .filter((w) => w.length > 2 && !/^\d+$/.test(w)),
  );

// Эталон — построчная расшифровка кадра. Строка с «#» — комментарий, в замер не
// идёт вовсе. Строка с «~» — то, что на листе есть, но позицией сметы не
// считается (итоги, нечитаемое имя, числа печатного бланка): её числа известны
// замеру — значит, они не выдумка, — но распознавания имени с модели не ждём.
// Остальные строки с числами — позиции: по ним и считается «строк распознано».
function truthOf(file) {
  const kept = readFileSync(file, "utf8")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"));
  const lines = kept
    .filter((l) => !l.startsWith("~") && numbersIn(l).length > 0)
    .map((l) => ({ words: words(l), numbers: numbersIn(l) }));
  return { lines, numbers: numbersIn(kept.join("\n")) };
}

// Совпадение имени работы: доля общих слов от более короткого имени.
function sameName(a, b) {
  if (!a.size || !b.size) return 0;
  let common = 0;
  for (const w of a) if (b.has(w)) common += 1;
  return common / Math.min(a.size, b.size);
}

// --- метрики ------------------------------------------------------------

function score(truth, answer) {
  const got = (answer.lines ?? []).map((l) => ({
    words: words(String(l.name ?? "")),
    numbers: [l.quantity, l.price, l.total].map(Number).filter((n) => Number.isFinite(n)),
  }));
  let wrong = 0;
  let invented = 0;
  let checked = 0;
  const used = new Set();
  const pairs = [];
  truth.lines.forEach((expected, ti) => {
    let best = -1;
    let bestScore = 0.5;
    got.forEach((line, i) => {
      const s = sameName(expected.words, line.words);
      if (!used.has(i) && s > bestScore) {
        bestScore = s;
        best = i;
      }
    });
    if (best === -1) return;
    used.add(best);
    pairs.push([ti, best]);
  });
  // Второй проход — по числам: строка, у которой сошлись количество, цена и
  // сумма, распознана, даже если имя написано иначе («гастроинтестин» вместо
  // «гастроинтестинал»). Иначе замер штрафует модель за букву, а не за смету.
  truth.lines.forEach((expected, ti) => {
    if (pairs.some(([t]) => t === ti)) return;
    const found = got.findIndex(
      (line, i) =>
        !used.has(i) &&
        line.numbers.length >= 2 &&
        line.numbers.every((n) => hasNumber(expected.numbers, n)),
    );
    if (found === -1) return;
    used.add(found);
    pairs.push([ti, found]);
  });
  const matched = pairs.length;
  for (const [ti, gi] of pairs) {
    for (const value of got[gi].numbers) {
      checked += 1;
      if (hasNumber(truth.lines[ti].numbers, value)) continue;
      if (hasNumber(truth.numbers, value)) wrong += 1;
      else invented += 1;
    }
  }
  // Числа в строках, которых на листе вообще нет, — тоже выдумка.
  got.forEach((line, i) => {
    if (used.has(i)) return;
    for (const value of line.numbers) {
      checked += 1;
      if (!hasNumber(truth.numbers, value)) invented += 1;
    }
  });
  return { expected: truth.lines.length, matched, wrong, invented, checked };
}

// --- вызов модели -------------------------------------------------------

const MIME = { ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png", ".webp": "image/webp" };

const keyOf = (candidate) => env[candidate.keyEnv ?? "OPENROUTER_API_KEY"];

async function ask(candidate, photo) {
  const key = keyOf(candidate);
  const bytes = readFileSync(photo);
  const mime = MIME[extname(photo).toLowerCase()];
  if (!mime) throw new Error(`не картинка: ${photo}`);
  const model = (candidate.model ?? "").replace(/\{(\w+)\}/g, (_, name) => env[name] ?? "");
  const started = Date.now();
  const base = env.LLM_PROBE_BASE ?? candidate.base ?? OPENROUTER;
  const res = await fetch(`${base}/chat/completions`, {
    method: "POST",
    headers: {
      authorization: `${candidate.scheme ?? "Bearer"} ${key}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      model,
      max_tokens: maxTokens,
      messages: [
        {
          role: "user",
          content: [
            { type: "text", text: PROMPT },
            { type: "image_url", image_url: { url: `data:${mime};base64,${bytes.toString("base64")}` } },
          ],
        },
      ],
    }),
    signal: AbortSignal.timeout(180_000),
  });
  const body = await res.text();
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${body.slice(0, 200)}`);
  const json = JSON.parse(body);
  const text = json.choices?.[0]?.message?.content ?? "";
  return { text, usage: json.usage ?? {}, seconds: (Date.now() - started) / 1000, bytes: bytes.length };
}

// Модели любят обернуть JSON в ```json — снимаем обёртку, дальше строго.
function parseAnswer(text) {
  const fenced = text.match(/```(?:json)?\s*([\s\S]*?)```/);
  const raw = (fenced ? fenced[1] : text).trim();
  const start = raw.indexOf("{");
  if (start === -1) throw new Error("в ответе нет JSON");
  return JSON.parse(raw.slice(start, raw.lastIndexOf("}") + 1));
}

// --- прогон -------------------------------------------------------------

async function priceList() {
  try {
    const res = await fetch(`${OPENROUTER}/models`, { signal: AbortSignal.timeout(30_000) });
    const map = {};
    for (const m of (await res.json()).data) map[m.id] = m.pricing;
    return map;
  } catch {
    return {};
  }
}

async function zdrModels() {
  try {
    const res = await fetch(`${OPENROUTER}/endpoints/zdr`, { signal: AbortSignal.timeout(30_000) });
    return new Set((await res.json()).data.map((e) => e.model_id));
  } catch {
    return new Set();
  }
}

function classes(dir) {
  if (!existsSync(dir)) throw new Error(`нет корпуса: ${dir} (см. docs/llm-data.md)`);
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory() && !e.name.startsWith("."))
    .map((e) => ({
      name: e.name,
      photos: readdirSync(join(dir, e.name))
        .filter((f) => MIME[extname(f).toLowerCase()])
        .slice(0, limit)
        .map((f) => join(dir, e.name, f)),
    }));
}

const pct = (part, whole) => (whole ? `${((part / whole) * 100).toFixed(0)}%` : "—");

// Цена всего прогона по строке таблицы: OpenRouter отдаёт факт в usage.cost,
// прайс-лист — запасной вариант (например, для провайдера без такого поля).
function costOf(row, prices) {
  if (row.cost) return row.cost;
  const p = prices[row.model] ?? {};
  if (!p.prompt) return 0;
  return row.promptTokens * +p.prompt + row.completionTokens * +p.completion;
}

function report(results, prices, zdr, empty) {
  const out = [];
  let total = 0;
  for (const [cls, rows] of Object.entries(results)) {
    out.push(`\n### ${cls} (${rows[0]?.photos ?? 0} кадров)\n`);
    out.push("| модель | ZDR | строк | чисел неверно | выдумано | токенов/фото | $/фото | с/фото | сбои |");
    out.push("|---|---|---|---|---|---|---|---|---|");
    for (const r of rows) {
      if (r.noKey) {
        out.push(`| ${r.id} | — | нет ключа ${r.keyEnv} в .env.probe — кандидат пропущен ||||||`);
        continue;
      }
      const cost = costOf(r, prices);
      total += cost;
      out.push(
        `| ${r.id} | ${zdr.has(r.model) ? "да" : r.model.startsWith("gpt://") ? "РФ" : "нет"} ` +
          `| ${pct(r.matched, r.expected)} | ${pct(r.wrong, r.checked)} | ${r.invented} ` +
          `| ${r.ok ? Math.round((r.promptTokens + r.completionTokens) / r.ok) : "—"} ` +
          `| ${cost ? (cost / r.ok).toFixed(4) : "—"} | ${r.ok ? (r.seconds / r.ok).toFixed(1) : "—"} | ${r.fails.length} |`,
      );
    }
    for (const r of rows) for (const f of r.fails) out.push(`\n- сбой ${r.id} на ${f}`);
  }
  for (const cls of empty) out.push(`\n### ${cls}: кадров нет — класс ждёт съёмки`);
  out.push(`\nПрогон стоил $${total.toFixed(4)} (по счёту провайдера).`);
  out.push(
    "Планка F0: строк ≥90%, чисел неверно ≤1%, выдумано = 0. " +
      "Вердикт считается по каждому классу отдельно.",
  );
  return out.join("\n");
}

const [prices, zdr] = await Promise.all([priceList(), zdrModels()]);
const candidates = CANDIDATES.filter((c) => !only.length || only.includes(c.id));
const outDir = join(corpusDir, ".probe-out");
mkdirSync(outDir, { recursive: true });
const results = {};
const empty = [];

for (const cls of classes(corpusDir)) {
  if (!cls.photos.length) {
    empty.push(cls.name);
    continue;
  }
  results[cls.name] = [];
  for (const candidate of candidates) {
    const row = {
      id: candidate.id,
      model: candidate.model,
      photos: cls.photos.length,
      // Нет ключа — кандидат пропускается строкой в таблице, а не падением.
      noKey: keyOf(candidate) ? false : true,
      keyEnv: candidate.keyEnv ?? "OPENROUTER_API_KEY",
      expected: 0, matched: 0, wrong: 0, invented: 0, checked: 0,
      promptTokens: 0, completionTokens: 0, seconds: 0, ok: 0, cost: 0, fails: [],
    };
    for (const photo of row.noKey ? [] : cls.photos) {
      process.stderr.write(`${cls.name}/${basename(photo)} → ${candidate.id}\n`);
      try {
        const { text, usage, seconds, bytes } = await ask(candidate, photo);
        const answer = parseAnswer(text);
        const s = score(truthOf(photo.replace(/\.\w+$/, ".truth.txt")), answer);
        for (const k of ["expected", "matched", "wrong", "invented", "checked"]) row[k] += s[k];
        row.promptTokens += usage.prompt_tokens ?? 0;
        row.completionTokens += usage.completion_tokens ?? 0;
        row.cost += usage.cost ?? 0;
        row.seconds += seconds;
        row.ok += 1;
        writeFileSync(
          join(outDir, `${candidate.id}__${cls.name}__${basename(photo)}.json`),
          JSON.stringify({ bytes, seconds, usage, score: s, answer }, null, 1),
        );
      } catch (e) {
        row.fails.push(`${basename(photo)}: ${e.message}`);
      }
    }
    results[cls.name].push(row);
  }
}

console.log(report(results, prices, zdr, empty));
