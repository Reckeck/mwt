import { describe, expect, it } from 'vitest'

import { guestDraft, hasGuestDetails } from './guest-details'

const details = (patch: Partial<GuestDetails>): GuestDetails => ({
  phone: '',
  messenger: '',
  requisites: [],
  note: '',
  ...patch,
})

describe('hasGuestDetails', () => {
  it('is false for nothing or blanks, true for any filled field', () => {
    expect(hasGuestDetails(null)).toBe(false)
    expect(hasGuestDetails(details({ phone: '  ' }))).toBe(false)
    expect(hasGuestDetails(details({ messenger: '@olya' }))).toBe(true)
  })
})

describe('guestDraft', () => {
  it('trims the name and drops empty details', () => {
    expect(guestDraft('  Оля ', details({ note: ' ' }))).toEqual({
      id: null,
      name: 'Оля',
      contact_id: null,
      details: null,
    })
  })

  it('keeps trimmed details and the chosen id', () => {
    const draft = guestDraft('Оля', details({ phone: ' +7 912 ' }), 'p1')
    expect(draft.id).toBe('p1')
    expect(draft.details?.phone).toBe('+7 912')
  })
})
