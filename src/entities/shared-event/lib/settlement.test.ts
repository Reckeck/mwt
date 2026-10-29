import { describe, expect, it } from 'vitest'

import {
  ME,
  absolute,
  canUseWallet,
  canWriteOff,
  exceeds,
  originalDebt,
  personStatus,
  rowAction,
  settleDirection,
  sign,
} from './settlement'

const person = (extra: Partial<PersonView>): PersonView => ({
  id: 'petya',
  name: 'Петя',
  kind: 'contact',
  contact_id: 'c-petya',
  has_details: false,
  is_organizer: false,
  share: '1735',
  paid: '0',
  moved: '0',
  written_off: '0',
  saldo: '1735',
  due_at: null,
  ...extra,
})

const view = (organizer: string, people: PersonView[], participate = true): SharedEventView =>
  ({ event: { organizer, i_participate: participate }, people }) as unknown as SharedEventView

describe('sign, absolute, exceeds', () => {
  it('reads decimals', () => {
    expect(sign('1735')).toBe(1)
    expect(sign('-0.50')).toBe(-1)
    expect(sign('0')).toBe(0)
    expect(sign('0.00')).toBe(0)
    expect(sign('')).toBe(0)
    expect(absolute('-685')).toBe('685')
    expect(exceeds('1735.01', '1735')).toBe(true)
    expect(exceeds('1735', '1735')).toBe(false)
  })

  it('names the direction by the sign of the saldo', () => {
    expect(settleDirection('10')).toBe('to_organizer')
    expect(settleDirection('-10')).toBe('from_organizer')
    expect(settleDirection('0')).toBeNull()
  })
})

describe('personStatus', () => {
  it('waits, then goes partial, returned or closed', () => {
    expect(personStatus(person({}))).toBe('waiting')
    expect(personStatus(person({ moved: '1000', saldo: '685' }))).toBe('partial')
    expect(personStatus(person({ moved: '1735', saldo: '0' }))).toBe('returned')
    expect(personStatus(person({ moved: '1000', written_off: '685', saldo: '0' }))).toBe('closed')
    expect(personStatus(person({ saldo: '0', moved: '0' }))).toBe('even')
    expect(personStatus(person({ is_organizer: true }))).toBe('organizer')
  })

  it('counts the original debt back from what is left', () => {
    expect(originalDebt(person({ moved: '1000', saldo: '685' }))).toBe('1685')
    expect(originalDebt(person({ moved: '1000', written_off: '685', saldo: '0' }))).toBe('1685')
  })
})

describe('rowAction', () => {
  const me = person({ id: ME, name: '', contact_id: null, kind: 'me', saldo: '1850' })
  const anya = person({ id: 'anya', name: 'Аня', contact_id: 'c-anya', is_organizer: true, saldo: '-5550' })
  const peter = person({})

  it('organizer is me: receive from those who owe, give to those I owe', () => {
    const mine = view(ME, [
      person({ id: ME, is_organizer: true, saldo: '-100' }),
      peter,
      person({ id: 'x', saldo: '-50' }),
    ])
    expect(rowAction(mine, peter)).toBe('receive')
    expect(rowAction(mine, person({ id: 'x', saldo: '-50' }))).toBe('give')
    expect(rowAction(mine, person({ saldo: '0' }))).toBeNull()
  })

  it('organizer is someone else: only my row pays, nobody else touches my wallets', () => {
    const theirs = view('anya', [me, peter, anya])
    expect(rowAction(theirs, me)).toBe('pay')
    expect(rowAction(theirs, peter)).toBeNull()
    expect(rowAction(theirs, anya)).toBeNull()
    expect(rowAction(theirs, { ...me, saldo: '-30' })).toBe('receive')
  })

  it('an event I do not take part in has no wallet actions', () => {
    expect(rowAction(view(ME, [peter], false), peter)).toBeNull()
    expect(canUseWallet(view(ME, [peter], false), peter)).toBe(false)
  })
})

describe('wallet and write-off rules', () => {
  it('a guest is set-off only', () => {
    const guest = person({ id: 'olya', kind: 'guest', contact_id: null })
    expect(canUseWallet(view(ME, [guest]), guest)).toBe(false)
    expect(canUseWallet(view(ME, [person({})]), person({}))).toBe(true)
  })

  it('writing off needs me as organizer and a debt to me', () => {
    expect(canWriteOff(view(ME, []), person({}))).toBe(true)
    expect(canWriteOff(view(ME, []), person({ saldo: '-5' }))).toBe(false)
    expect(canWriteOff(view('anya', []), person({}))).toBe(false)
  })
})
