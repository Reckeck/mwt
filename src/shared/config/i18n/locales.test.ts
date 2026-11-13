import { describe, expect, it } from 'vitest'

import en from './locales/en.json'
import ru from './locales/ru.json'

/** Messages written as `one | few | many`; anywhere else a `|` cuts the text short. */
const PLURAL_KEYS = /^(reminder\.unit\.|reminder\.repeatEveryN\.|adjustment\.submit$)/

describe('locales', () => {
  it('has the same keys in both languages', () => {
    expect(Object.keys(ru).sort()).toEqual(Object.keys(en).sort())
  })

  it('keeps « | » out of plain sentences', () => {
    const piped = Object.entries({ ...en, ...ru })
      .filter(([key, value]) => typeof value === 'string' && value.includes(' | ') && !PLURAL_KEYS.test(key))
      .map(([key]) => key)
    expect(piped).toEqual([])
  })
})
