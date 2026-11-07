import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { errorCodeText } from './error-code-text'

const { t, te } = i18n.global

describe('errorCodeText', () => {
  it('translates a known code', () => {
    expect(errorCodeText('SECRET', t, te)).toBe(t('errors.SECRET'))
  })

  it('falls back to the unknown-error text for a code without text', () => {
    expect(errorCodeText('NO_SUCH_CODE', t, te)).toBe(t('errors.UNKNOWN'))
  })

  it.each(['IO', 'ALREADY_EXISTS', 'UNSUPPORTED', 'SERIALIZE', 'DESERIALIZE', 'VAULT_BUSY', 'VAULT_TOO_HEAVY'])(
    'has its own text for %s',
    (code) => {
      expect(errorCodeText(code, t, te)).not.toBe(t('errors.UNKNOWN'))
      expect(te(`errors.${code}`)).toBe(true)
    },
  )
})
