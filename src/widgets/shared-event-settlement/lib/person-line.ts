import { ME, absolute, originalDebt, sign } from '@/entities/shared-event'

/** Who the line is good or bad news for: green is owed to me, red is owed by me, the rest is only information. */
export type LineTone = 'owes-me' | 'i-owe' | 'muted'

/** The wording of a person's debt: an i18n key under `sharedEvents.settle.line.` and the sums it names. */
export interface PersonLine {
  key:
    | 'owes'
    | 'iOwe'
    | 'iOweThem'
    | 'owesMe'
    | 'owesOrganizer'
    | 'receives'
    | 'left'
    | 'returned'
    | 'returnedWritten'
    | 'writtenOff'
    | 'even'
    | 'leads'
  tone: LineTone
  amount?: string
  total?: string
  returned?: string
  writtenOff?: string
  /** The organizer's name, for «вы должны Ане». */
  name?: string
}

/** What the table says about one person's debt, from my side (the event's money runs through the organizer). */
export function personLine(view: SharedEventView, person: PersonView): PersonLine {
  const organizer = view.people.find((entry) => entry.is_organizer)
  const byMe = view.event.organizer === ME
  const direction = sign(person.saldo)
  const moved = sign(person.moved) !== 0
  const written = sign(person.written_off) > 0
  const name = organizer?.name
  const left = absolute(person.saldo)

  if (person.is_organizer) {
    return direction < 0 ? { key: 'receives', tone: 'muted', amount: left } : { key: 'leads', tone: 'muted' }
  }

  if (direction === 0) {
    const returned = absolute(person.moved)
    if (written && moved) return { key: 'returnedWritten', tone: 'muted', returned, writtenOff: person.written_off }
    if (written) return { key: 'writtenOff', tone: 'muted', writtenOff: person.written_off }
    return moved ? { key: 'returned', tone: 'muted', amount: returned } : { key: 'even', tone: 'muted' }
  }

  // Positive: the person owes the organizer; negative: the organizer owes the person.
  const mine = byMe || person.id === ME
  const toMe = byMe ? direction > 0 : person.id === ME && direction < 0
  const tone: LineTone = !mine ? 'muted' : toMe ? 'owes-me' : 'i-owe'

  if (moved || written) return { key: 'left', tone, amount: left, total: absolute(originalDebt(person)) }
  if (direction > 0) {
    if (byMe) return { key: 'owes', tone, amount: left }
    return person.id === ME ? { key: 'iOwe', tone, amount: left, name } : { key: 'owesOrganizer', tone, amount: left }
  }
  if (byMe) return { key: 'iOweThem', tone, amount: left }
  return person.id === ME ? { key: 'owesMe', tone, amount: left, name } : { key: 'receives', tone, amount: left }
}
