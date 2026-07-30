import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

// без vitest globals авто-очистка Testing Library не включается — чистим сами,
// иначе DOM предыдущего теста протекает в следующий
afterEach(cleanup)
