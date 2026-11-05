import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { anyErrorMessage } from './vault-error'

describe('anyErrorMessage', () => {
  it('shows the translated text of CONTACT_IN_USE without the English backend tail', () => {
    const text = anyErrorMessage('CONTACT_IN_USE: the contact is used by operations', (key) => i18n.global.t(key))
    expect(text).toBe(i18n.global.t('errors.CONTACT_IN_USE'))
    expect(text).not.toContain('the contact is used')
  })
})
