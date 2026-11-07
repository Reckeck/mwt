import { describe, expect, it } from 'vitest'
import { createI18n } from 'vue-i18n'

import { LOCALE_PARTS } from '@/shared/config/i18n'

import { duplicateLocaleKeys, mergeLocale } from './parts'

const PARTS = {
  './locales/parts/demo.ru.json': { 'demo.title': 'Заголовок' },
  './locales/parts/demo.en.json': { 'demo.title': 'Title' },
}

describe('locale parts', () => {
  it('finds the part files of every owner, in both languages', () => {
    const owners = [
      'security-autolock',
      'security-keys',
      'security-levels',
      'security-biometric',
      'security-onboarding',
    ]
    const files = owners.flatMap((owner) => [`./locales/parts/${owner}.ru.json`, `./locales/parts/${owner}.en.json`])
    expect(Object.keys(LOCALE_PARTS)).toEqual(expect.arrayContaining(files))
    // The messages themselves, not a module wrapping them.
    for (const messages of Object.values(LOCALE_PARTS)) expect(messages).not.toHaveProperty('default')
  })

  it('shows a key of a part through t()', () => {
    const { global } = createI18n({
      legacy: false,
      locale: 'ru',
      messages: { ru: mergeLocale({ 'main.title': 'Главная' }, 'ru', PARTS) },
    })
    expect(global.t('demo.title')).toBe('Заголовок')
    expect(global.t('main.title')).toBe('Главная')
  })

  it('takes only the parts of the language', () => {
    expect(mergeLocale({}, 'en', PARTS)).toEqual({ 'demo.title': 'Title' })
  })

  it('names a key both a main file and a part define', () => {
    expect(duplicateLocaleKeys({ 'demo.title': 'Занято' }, 'ru', PARTS)).toEqual([
      'i18n key "demo.title" is in both ru.json and demo.ru.json',
    ])
  })

  it('names a key two parts define', () => {
    const parts = { ...PARTS, './locales/parts/other.ru.json': { 'demo.title': 'Ещё раз' } }
    expect(duplicateLocaleKeys({}, 'ru', parts)).toEqual([
      'i18n key "demo.title" is in both demo.ru.json and other.ru.json',
    ])
  })

  it('accepts the same key in the two languages of a part', () => {
    expect(duplicateLocaleKeys({}, 'ru', PARTS)).toEqual([])
  })
})
