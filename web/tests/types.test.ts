import { expect, it } from 'vitest'
import { formatDate, formatBytes } from '../src/types'

it('formats file dates as a stable local date and minute without wrapping seconds', () => {
  expect(formatDate('invalid-date')).toBe('invalid-date')
  const date = new Date(2025, 11, 7, 23, 26, 54)
  expect(formatDate(date.toISOString())).toBe('2025/12/07 23:26')
})

it('formats quota byte values with binary units and bounded precision', () => {
  expect(formatBytes(1024 ** 3)).toBe('1 GB')
  expect(formatBytes(1024 ** 4)).toBe('1 TB')
  expect(formatBytes(1536)).toBe('1.5 KB')
})
