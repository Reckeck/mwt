/** Commands: shared events, their people and their items (purchases and settling: commands-shared-settle.ts). */

import { mockTransactions } from './demo'
import { mockContacts } from './ledger'
import { resolve } from './core'
import type { MockCommands } from './core'
import { ME } from './shared-events/engine'
import { fail, findEvent, mockSharedEvents } from './shared-events/state'
import { contactEvents, eventView, listSummaries, shareText, txLinks } from './shared-events/view'
import { txId } from '@/shared/lib/transactions'

const STEPS = ['0.01', '1', '10', '50', '100']
const DAY = /^\d{4}-\d{2}-\d{2}$/

export const txExists = (id: string) => mockTransactions.some((tx) => txId(tx) === id)

export const viewOf = (event: SharedEvent) => resolve(eventView(event, txExists))

/** The event, or a rejected `MISSING` the caller returns. */
export function eventOrFail(id: string): SharedEvent | Promise<never> {
  return findEvent(id) ?? fail('MISSING', `shared event ${id}`)
}

function checkDates(starts: string | null, ends: string | null): [string | null, string | null] | string {
  const start = starts?.trim() || null
  const end = ends?.trim() || null
  if ((start && !DAY.test(start)) || (end && !DAY.test(end))) return 'a date is YYYY-MM-DD'
  if (end && !start) return 'the end date needs a start date'
  if (start && end && end < start) return 'the event cannot end before it starts'
  return [start, end && end !== start ? end : null]
}

function buildParticipant(draft: SharedParticipantDraft, others: Participant[]): Participant | string {
  const id = draft.id?.trim() || crypto.randomUUID()
  if (id === ME || others.some((person) => person.id === id)) return `participant id ${id} is taken`
  if (draft.contact_id) {
    const contact = mockContacts.find((entry) => entry.id === draft.contact_id)
    if (!contact) return `contact ${draft.contact_id}`
    if (others.some((person) => person.contact_id === contact.id)) return 'this contact already takes part'
    return { id, name: contact.name, contact_id: contact.id, details: null }
  }
  if (!draft.name.trim()) return 'a guest needs a name'
  const details = draft.details
  const filled =
    details && (details.phone.trim() || details.messenger.trim() || details.note.trim() || details.requisites.length)
  return { id, name: draft.name.trim(), contact_id: null, details: filled ? details : null }
}

function applyDraft(event: SharedEvent, draft: SharedEventDraft): string | null {
  const title = draft.title.trim()
  if (!title) return 'an event needs a title'
  const dates = checkDates(draft.starts_on, draft.ends_on)
  if (typeof dates === 'string') return dates
  const step = draft.rounding_step ?? '1'
  if (!STEPS.includes(step)) return 'the rounding step is 0.01, 1, 10, 50 or 100'
  const organizer = draft.organizer.trim() || ME
  if (organizer !== ME && !event.participants.some((person) => person.id === organizer))
    return `participant ${organizer}`
  Object.assign(event, {
    title,
    note: draft.note.trim(),
    starts_on: dates[0],
    ends_on: dates[1],
    currency: draft.currency.trim().toLowerCase() || 'rub',
    rounding_step: step,
    category_id: draft.category_id,
    organizer,
    i_participate: draft.i_participate,
  })
  return null
}

const bad = (message: string) =>
  fail(message.startsWith('participant') || message.startsWith('contact') ? 'MISSING' : 'INVALID_INPUT', message)

export const sharedEventCommands: MockCommands = {
  list_shared_events: ({ args }) =>
    resolve(listSummaries(Boolean((args?.filter as SharedEventFilter | null | undefined)?.include_archived))),
  get_shared_event: ({ args }) => {
    const event = eventOrFail(String(args?.id))
    return event instanceof Promise ? event : viewOf(event)
  },
  create_shared_event: ({ body }) => {
    const draft = body as SharedEventDraft
    const event: SharedEvent = {
      id: crypto.randomUUID(),
      title: '',
      created_at: new Date().toISOString(),
      note: '',
      archived: false,
      starts_on: null,
      ends_on: null,
      currency: 'rub',
      rounding_step: '1',
      category_id: null,
      organizer: ME,
      i_participate: true,
      participants: [],
      items: [],
      settlements: [],
      adjustments: [],
    }
    for (const person of draft.participants) {
      const built = buildParticipant(person, event.participants)
      if (typeof built === 'string') return bad(built)
      event.participants.push(built)
    }
    const problem = applyDraft(event, draft)
    if (problem) return bad(problem)
    mockSharedEvents.push(event)
    return viewOf(event)
  },
  update_shared_event: ({ body }) => {
    const input = body as UpdateSharedEventInput
    const event = eventOrFail(input.id)
    if (event instanceof Promise) return event
    const copy = structuredClone(event)
    const draft = { ...input.event, organizer: input.event.organizer || event.organizer }
    const problem = applyDraft(copy, draft)
    if (problem) return bad(problem)
    if (copy.currency !== event.currency && event.items.some((item) => item.purchases.length)) {
      return bad('the currency cannot change once something is bought')
    }
    if (copy.organizer !== event.organizer && event.settlements.length) {
      return bad('the organizer cannot change once people have settled')
    }
    Object.assign(event, copy)
    return viewOf(event)
  },
  delete_shared_event: ({ args }) => {
    const event = eventOrFail(String(args?.id))
    if (event instanceof Promise) return event
    const owned = new Set([
      ...event.items.flatMap((item) =>
        item.purchases.filter((purchase) => purchase.tx_owned).map((entry) => entry.tx_id),
      ),
      ...event.settlements.map((settlement) => settlement.tx_id),
    ])
    for (let index = mockTransactions.length - 1; index >= 0; index--) {
      if (owned.has(txId(mockTransactions[index]))) mockTransactions.splice(index, 1)
    }
    mockSharedEvents.splice(mockSharedEvents.indexOf(event), 1)
    return resolve({})
  },
  archive_shared_event: ({ body }) => {
    const input = body as ArchiveSharedEventInput
    const event = eventOrFail(input.id)
    if (event instanceof Promise) return event
    const status = eventView(event, txExists).status
    if (input.archived && status !== 'closed' && status !== 'planning')
      return bad('only a settled event can be archived')
    event.archived = input.archived
    return resolve({})
  },
  add_shared_participant: ({ body }) => {
    const input = body as SharedParticipantInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const built = buildParticipant(input.participant, event.participants)
    if (typeof built === 'string') return bad(built)
    event.participants.push(built)
    return viewOf(event)
  },
  update_shared_participant: ({ body }) => {
    const input = body as UpdateSharedParticipantInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const others = event.participants.filter((person) => person.id !== input.participant_id)
    const built = buildParticipant({ ...input.participant, id: null }, others)
    if (typeof built === 'string') return bad(built)
    const index = event.participants.findIndex((person) => person.id === input.participant_id)
    if (index < 0) return bad(`participant ${input.participant_id}`)
    event.participants[index] = { ...built, id: input.participant_id }
    return viewOf(event)
  },
  remove_shared_participant: ({ body }) => {
    const input = body as SharedParticipantRef
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const who = input.participant_id
    const busy =
      event.organizer === who ||
      event.items.some((item) => item.purchases.some((purchase) => purchase.buyer === who)) ||
      event.settlements.some((entry) => entry.participant_id === who || entry.to.includes(who)) ||
      event.adjustments.some((fix) => fix.participant_id === who)
    if (busy) return bad('this person organizes, paid, settled or has a fixed total in the event')
    event.participants = event.participants.filter((person) => person.id !== who)
    for (const item of event.items) {
      if ('equal' in item.split) item.split.equal.among = item.split.equal.among.filter((id) => id !== who)
      else delete item.split.by_units.units[who]
    }
    return viewOf(event)
  },
  make_shared_participant_contact: ({ body }) => {
    const input = body as SharedParticipantRef
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const guest = event.participants.find((person) => person.id === input.participant_id)
    if (!guest || guest.contact_id) return bad('this person is a contact already')
    const details = guest.details
    const accounts: ContactAccount[] = [
      ...(details?.phone
        ? [
            {
              id: crypto.randomUUID(),
              group: null,
              label: 'Телефон',
              kind: 'phone' as const,
              value: details.phone,
              currency: null,
              network: null,
              active: true,
            },
          ]
        : []),
      ...(details?.requisites ?? []).map((account) => ({ ...account, id: crypto.randomUUID(), active: true })),
    ]
    const contact: Contact = {
      id: crypto.randomUUID(),
      name: guest.name,
      kind: 'person',
      accounts,
      note: details?.note ?? '',
      archived: false,
      created_at: new Date().toISOString(),
      group: '',
    }
    mockContacts.push(contact)
    guest.contact_id = contact.id
    guest.details = null
    return resolve(contact)
  },
  get_contact_shared_events: ({ args }) => resolve(contactEvents(String(args?.contactId ?? args?.contact_id))),
  list_shared_tx_links: () => resolve(txLinks()),
  get_shared_event_share_text: ({ body }) => {
    const event = eventOrFail((body as SharedShareTextInput).event_id)
    return event instanceof Promise ? event : resolve(shareText(event))
  },
}
