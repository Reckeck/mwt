import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { anyErrorMessage, vaultErrorMessage } from './vault-error'

describe('anyErrorMessage', () => {
  it('shows the translated text of CONTACT_IN_USE without the English backend tail', () => {
    const text = anyErrorMessage('CONTACT_IN_USE: the contact is used by operations', (key) => i18n.global.t(key))
    expect(text).toBe(i18n.global.t('errors.CONTACT_IN_USE'))
    expect(text).not.toContain('the contact is used')
  })
})

describe('vaultErrorMessage', () => {
  it('tells a vault too heavy for this device apart from a wrong password', () => {
    const translate = (key: string) => i18n.global.t(key)
    const text = vaultErrorMessage('VAULT_TOO_HEAVY: its key needs 1024 MiB, this device can give 300 MiB', translate)
    expect(text).toBe(i18n.global.t('auth.vaultTooHeavy'))
    expect(text).not.toBe(i18n.global.t('auth.wrongPassword'))
  })
})
