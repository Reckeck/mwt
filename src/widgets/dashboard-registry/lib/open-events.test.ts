import { describe, expect, it } from 'vitest'

import { openEvents, totalsByCurrency } from './open-events'

const event = (extra: Partial<SharedEventSummary>): SharedEventSummary => ({
  id: 'e',
  title: 'Шашлыки',
  starts_on: null,
  ends_on: null,
  currency: 'rub',
  status: 'settling',
  archived: false,
  i_participate: true,
  people_count: 5,
  organizer_name: '',
  organizer_is_me: true,
  planned: '0',
  spent: '0',
  owed_to_me: '0',
  i_owe: '0',
  ...extra,
})

describe('open events', () => {
  it('keeps events with money between us and drops closed, archived and empty ones', () => {
    const list = [
      event({ id: 'a', owed_to_me: '2120' }),
      event({ id: 'b', status: 'closed', owed_to_me: '10' }),
      event({ id: 'c', archived: true, owed_to_me: '10' }),
      event({ id: 'd', status: 'in_progress' }),
      event({ id: 'e', i_owe: '1850', organizer_is_me: false }),
    ]
    expect(openEvents(list).map((row) => row.id)).toEqual(['a', 'e'])
  })

  it('sums per currency without mixing them', () => {
    const totals = totalsByCurrency([
      event({ owed_to_me: '2120.50' }),
      event({ owed_to_me: '100', i_owe: '5' }),
      event({ currency: 'usd', owed_to_me: '30' }),
    ])
    expect(totals).toEqual([
      { currency: 'rub', owedToMe: '2220.5', iOwe: '5' },
      { currency: 'usd', owedToMe: '30', iOwe: '0' },
    ])
  })
})
