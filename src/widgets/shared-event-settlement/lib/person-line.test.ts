import { describe, expect, it } from 'vitest'

import { personLine } from './person-line'

const person = (extra: Partial<PersonView>): PersonView => ({
  id: 'petya',
  name: 'Петя',
  kind: 'contact',
  contact_id: 'c',
  has_details: false,
  is_organizer: false,
  share: '0',
  paid: '0',
  moved: '0',
  written_off: '0',
  saldo: '1735',
  due_at: null,
  ...extra,
})

const view = (organizer: string, people: PersonView[]): SharedEventView =>
  ({ event: { organizer, i_participate: true }, people }) as unknown as SharedEventView

describe('personLine, I lead', () => {
  const organizer = person({ id: 'me', name: '', is_organizer: true, saldo: '-100' })

  it('says who owes and how much is left', () => {
    expect(personLine(view('me', [organizer]), person({}))).toMatchObject({
      key: 'owes',
      tone: 'owes-me',
      amount: '1735',
    })
    expect(personLine(view('me', [organizer]), person({ saldo: '-50' }))).toMatchObject({
      key: 'iOweThem',
      tone: 'i-owe',
      amount: '50',
    })
    expect(personLine(view('me', [organizer]), person({ moved: '1000', saldo: '685' }))).toMatchObject({
      key: 'left',
      amount: '685',
      total: '1685',
      tone: 'owes-me',
    })
  })

  it('reads a settled person', () => {
    expect(personLine(view('me', [organizer]), person({ moved: '1735', saldo: '0' }))).toMatchObject({
      key: 'returned',
      amount: '1735',
    })
    expect(
      personLine(view('me', [organizer]), person({ moved: '1000', written_off: '685', saldo: '0' })),
    ).toMatchObject({
      key: 'returnedWritten',
      returned: '1000',
      writtenOff: '685',
    })
    expect(personLine(view('me', [organizer]), person({ saldo: '0' }))).toMatchObject({ key: 'even' })
  })
})

describe('personLine, someone else leads', () => {
  const anya = person({ id: 'anya', name: 'Аня', is_organizer: true, saldo: '-5550' })
  const me = person({ id: 'me', name: '', saldo: '1850' })

  it('shows what I owe the organizer and what others owe them', () => {
    const event = view('anya', [me, anya])
    expect(personLine(event, me)).toMatchObject({ key: 'iOwe', tone: 'i-owe', name: 'Аня' })
    expect(personLine(event, person({}))).toMatchObject({ key: 'owesOrganizer', tone: 'muted' })
    expect(personLine(event, anya)).toMatchObject({ key: 'receives', amount: '5550' })
  })
})
