// Проверка «живого» поведения страницы без пачки скриншотов.
// Инструментирует страницу (Web Animations API + MutationObserver + rAF),
// опционально кликает по селектору и печатает компактный JSON:
// какие анимации сработали (свойство, длительность, easing), что изменилось
// в DOM, были ли долгие кадры (дёрганность), какие запросы ушли.
// Кадры до/в момент/после лежат в test-results/motion/.
//
// Использование (из web/):
//   pnpm motion <url> [селектор-для-клика]
//   pnpm motion --self-test

import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { chromium } from '@playwright/test'

const OUT_DIR = path.join(import.meta.dirname, '..', 'test-results', 'motion')
const SETTLE_MS = 1500 // ponytail: фиксированное окно наблюдения; сделать флагом, если анимации длиннее

// Выполняется внутри страницы до её загрузки: вешает наблюдателей.
function instrument() {
  const seen = new Set()
  const m = { added: {}, removed: {}, attrs: {}, anims: [], frames: [] }
  const desc = (el) => {
    if (!el || !el.tagName) return 'text'
    const cls =
      typeof el.className === 'string' ? el.className.trim().split(/\s+/).slice(0, 2).join('.') : ''
    return el.tagName.toLowerCase() + (cls ? '.' + cls : '')
  }
  const bump = (obj, key) => {
    obj[key] = (obj[key] || 0) + 1
  }
  new MutationObserver((muts) => {
    for (const mu of muts) {
      for (const n of mu.addedNodes) bump(m.added, desc(n))
      for (const n of mu.removedNodes) bump(m.removed, desc(n))
      if (mu.type === 'attributes') bump(m.attrs, desc(mu.target) + '[' + mu.attributeName + ']')
    }
  }).observe(document, { subtree: true, childList: true, attributes: true })
  const tick = (t) => {
    m.frames.push(t)
    for (const a of document.getAnimations()) {
      const el = a.effect && a.effect.target
      const what = a.transitionProperty || a.animationName || 'js-animation'
      const key = what + '@' + desc(el)
      if (seen.has(key)) continue
      seen.add(key)
      const timing = a.effect.getTiming()
      const kf = a.effect.getKeyframes ? a.effect.getKeyframes() : []
      // easing транзишена Chrome не кладёт в keyframes — берём из computed style
      const styleEasing =
        el && a.transitionProperty ? getComputedStyle(el).transitionTimingFunction : ''
      m.anims.push({
        target: desc(el),
        what,
        durationMs: timing.duration,
        easing: styleEasing || (kf[0] && kf[0].easing) || timing.easing,
      })
    }
    requestAnimationFrame(tick)
  }
  requestAnimationFrame(tick)
  const top = (obj) =>
    Object.fromEntries(
      Object.entries(obj)
        .sort((a, b) => b[1] - a[1])
        .slice(0, 10),
    )
  window.__motion = {
    reset() {
      m.added = {}
      m.removed = {}
      m.attrs = {}
      m.anims.length = 0
      m.frames.length = 0
      seen.clear()
    },
    collect() {
      const gaps = m.frames.slice(1).map((t, i) => t - m.frames[i])
      return {
        anims: m.anims.slice(0, 20),
        dom: { added: top(m.added), removed: top(m.removed), attrs: top(m.attrs) },
        longFrames: gaps.filter((g) => g > 32).length,
      }
    },
  }
}

async function run(url, selector) {
  mkdirSync(OUT_DIR, { recursive: true })
  const shot = (name) => path.join(OUT_DIR, name + '.png')
  // caret: 'initial' — иначе Playwright прячет курсор, мутируя style инпутов,
  // и отчёт видит ложный DOM-шум
  const shotOpts = (name) => ({ path: shot(name), caret: 'initial' })
  const browser = await chromium.launch()
  const page = await browser.newPage()
  const errors = []
  const requests = []
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text().slice(0, 200))
  })
  await page.addInitScript(instrument)
  await page.goto(url, { waitUntil: 'networkidle' })
  page.on('request', (r) => requests.push(r.method() + ' ' + new URL(r.url()).pathname))
  await page.screenshot(shotOpts('before'))
  if (selector) {
    await page.evaluate(() => window.__motion.reset())
    await page.click(selector)
  }
  await page.waitForTimeout(150)
  await page.screenshot(shotOpts('mid'))
  await page.waitForTimeout(SETTLE_MS - 150)
  await page.screenshot(shotOpts('after'))
  const data = await page.evaluate(() => window.__motion.collect())
  await browser.close()

  const domChanges =
    Object.keys(data.dom.added).length +
    Object.keys(data.dom.removed).length +
    Object.keys(data.dom.attrs).length
  // ponytail: грубая эвристика; точное сопоставление «узел ↔ его анимация» — если понадобится
  const verdict =
    data.anims.length && domChanges
      ? 'animated'
      : domChanges
        ? 'abrupt (изменения без анимаций)'
        : data.anims.length
          ? 'animated'
          : 'static (ничего не изменилось)'
  return {
    action: selector ? 'click ' + selector : 'page load',
    verdict,
    smooth: data.longFrames <= 2 ? 'да' : 'нет, долгих кадров: ' + data.longFrames,
    ...data,
    requests: requests.slice(0, 10),
    consoleErrors: errors.slice(0, 5),
    screenshots: ['before', 'mid', 'after'].map((n) => shot(n)),
  }
}

async function selfTest() {
  const html = `<button id="b" onclick="
      const d = document.createElement('div'); d.className = 'box';
      document.body.appendChild(d);
      requestAnimationFrame(() => requestAnimationFrame(() => d.classList.add('in')))
    ">Go</button>
    <style>.box{opacity:0;width:50px;height:50px;background:teal;
      transition:opacity .3s ease-out}.box.in{opacity:1}</style>`
  const out = await run('data:text/html,' + encodeURIComponent(html), '#b')
  console.log(JSON.stringify(out, null, 1))
  if (!out.anims.some((a) => a.what === 'opacity')) throw new Error('транзишен не пойман')
  if (!out.dom.added['div.box']) throw new Error('добавленный узел не пойман')
  console.log('self-test: ok')
}

const [url, selector] = process.argv.slice(2)
if (url === '--self-test') {
  await selfTest()
} else if (url) {
  console.log(JSON.stringify(await run(url, selector), null, 1))
} else {
  console.error('Использование: pnpm motion <url> [селектор] | pnpm motion --self-test')
  process.exit(1)
}
