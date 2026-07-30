// Локали не могут разъехаться: у всех языков одинаковый набор ключей.
import { describe, expect, test } from 'vitest'
import { resources } from './i18n'

// множественные суффиксы i18next зависят от языка (ru: few/many, en: other),
// поэтому сравниваем базовые ключи
function baseKeys(dict: Record<string, string>): Set<string> {
  return new Set(Object.keys(dict).map((k) => k.replace(/_(one|few|many|other|zero|two)$/, '')))
}

describe('локали', () => {
  const langs = Object.keys(resources) as (keyof typeof resources)[]
  const reference = baseKeys(resources[langs[0]].translation)

  test.each(langs.slice(1))('набор ключей %s совпадает с %s', (lang) => {
    const keys = baseKeys(resources[lang].translation)
    expect([...keys].sort()).toEqual([...reference].sort())
  })

  test('русский словарь содержит все формы склонения количества', () => {
    const ru = resources.ru.translation as Record<string, string>
    for (const form of ['one', 'few', 'many']) {
      expect(ru[`estimates.recognized_${form}`], `estimates.recognized_${form}`).toBeDefined()
    }
  })
})
