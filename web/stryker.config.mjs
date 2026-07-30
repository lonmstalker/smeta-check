// Мутационные тесты фронта (StrykerJS). Запуск — scripts/mutants.sh
// (make mutants), после полной реализации фичи. Диагностика дыр в тестах,
// не гейт: порогов, роняющих сборку, нет.
export default {
  testRunner: 'vitest',
  // pnpm-раскладка ломает автопоиск по глобу @stryker-mutator/* — явно
  plugins: ['@stryker-mutator/vitest-runner'],
  // Наш typescript@7 (tsgo) не имеет старого API, которым Stryker
  // переписывает tsconfig для сэндбокса, — а переписывать и нечего:
  // tsconfig самодостаточен (paths относительные, алиасы резолвит vite).
  // Несуществующий путь заставляет Stryker пропустить этот шаг.
  tsconfigFile: 'missing-on-purpose.json',
  // на мутанта гоняются только тесты, покрывающие мутированную строку
  coverageAnalysis: 'perTest',
  // повторные прогоны переиспользуют прошлые результаты
  incremental: true,
  mutate: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.test.{ts,tsx}',
    '!src/api/schema.d.ts', // генерённое
    '!src/components/ui/**', // вендорное
  ],
  // строки в JSX (css-классы, ключи локалей) — эквивалентные мутанты, шум
  mutator: { excludedMutations: ['StringLiteral'] },
  // щадящий режим для ноутбука: не больше четырёх воркеров
  concurrency: 4,
  reporters: ['clear-text', 'progress', 'html'],
}
