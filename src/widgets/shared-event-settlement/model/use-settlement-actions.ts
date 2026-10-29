import { type MaybeRefOrGetter, computed, reactive, ref, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { Clock, HandCoins, Scale, Shuffle, Undo2 } from 'lucide-vue-next'

import {
  type RowAction,
  absolute,
  canUseWallet,
  canWriteOff,
  reassignTargets,
  rowAction,
  sign,
  sumDecimals,
} from '@/entities/shared-event'
import { formatDayMonthShort, formatList, formatMoney } from '@/shared/lib/format'
import type { UiAction } from '@/shared/ui/context-menu'

type SettleKind = 'received' | 'gave' | 'offset'

/** The labels of the row's main button: it takes the money in, hands it out, or pays my share. */
const ACTION_KIND: Record<RowAction, SettleKind | 'paid'> = { receive: 'received', give: 'gave', pay: 'paid' }

/**
 * What the settlement section can do with a person: the main button, the «⋯» list, and the state of
 * the dialogs those open (one person at a time, so a single set of dialogs serves every row).
 */
export function useSettlementActions(view: MaybeRefOrGetter<SharedEventView | undefined>) {
  const { t } = useI18n()

  const settle = reactive({ open: false, personId: '', kind: 'received' as SettleKind })
  const remainder = reactive({ open: false, personId: '', kind: 'write_off' as 'write_off' | 'reassign' })
  const due = reactive({ open: false, personId: '' })
  const pay = ref(false)
  const share = ref(false)

  const personOf = (id: string) => toValue(view)?.people.find((person) => person.id === id) ?? null
  const settlePerson = computed(() => personOf(settle.personId))
  const remainderPerson = computed(() => personOf(remainder.personId))
  const duePerson = computed(() => personOf(due.personId))
  const withWallet = computed(() => {
    const event = toValue(view)
    const person = settlePerson.value
    return Boolean(event && person && settle.kind !== 'offset' && canUseWallet(event, person))
  })

  function openSettle(person: PersonView, kind: SettleKind) {
    Object.assign(settle, { open: true, personId: person.id, kind })
  }

  /** The row's main button; `null` where money of mine is not involved. */
  function primaryOf(person: PersonView): { action: RowAction; label: string } | null {
    const event = toValue(view)
    const action = event ? rowAction(event, person) : null
    return action ? { action, label: t(`sharedEvents.settle.primary.${action}`) } : null
  }

  function runPrimary(person: PersonView) {
    const action = primaryOf(person)?.action
    if (!action) return
    if (action === 'pay') pay.value = true
    else openSettle(person, ACTION_KIND[action] as SettleKind)
  }

  /** «⋯» of a row: more of the same, set-off, a term, write-off, shifting the rest onto others. */
  function actionsOf(person: PersonView): UiAction[] {
    const event = toValue(view)
    if (!event || person.is_organizer || sign(person.saldo) === 0) return []
    const decimals = sumDecimals(event)
    // money: plain — the label and the hint of a menu item are strings
    const rest = formatMoney(absolute(person.saldo), { currency: event.event.currency, decimals })
    const main = primaryOf(person)
    const partial = sign(person.moved) !== 0 || sign(person.written_off) > 0
    const others = reassignTargets(event, person).map((entry) => entry.name || t('sharedEvents.me'))
    const writeOff = canWriteOff(event, person)
    return [
      {
        id: 'more',
        label: main ? t(`sharedEvents.settle.more.${main.action}`) : '',
        icon: HandCoins,
        hidden: !main || !partial,
        run: () => runPrimary(person),
      },
      {
        id: 'offset',
        label: t('sharedEvents.settle.offset'),
        hint: t('sharedEvents.settle.offsetHint'),
        icon: Scale,
        run: () => openSettle(person, 'offset'),
      },
      {
        id: 'due',
        label: t('sharedEvents.due.action'),
        hint: person.due_at ? t('sharedEvents.due.until', { date: formatDayMonthShort(person.due_at) }) : undefined,
        icon: Clock,
        hidden: !person.contact_id || person.id === 'me',
        run: () => Object.assign(due, { open: true, personId: person.id }),
      },
      {
        id: 'write-off',
        label: t('sharedEvents.remainder.write_off.action', { amount: rest }),
        icon: Undo2,
        separated: true,
        hidden: !writeOff,
        run: () => Object.assign(remainder, { open: true, personId: person.id, kind: 'write_off' }),
      },
      {
        id: 'reassign',
        label: t('sharedEvents.remainder.reassign.action'),
        hint: t('sharedEvents.remainder.reassign.hint', { amount: rest, names: formatList(others) }),
        icon: Shuffle,
        hidden: !writeOff,
        run: () => Object.assign(remainder, { open: true, personId: person.id, kind: 'reassign' }),
      },
    ]
  }

  return {
    settle,
    settlePerson,
    withWallet,
    remainder,
    remainderPerson,
    due,
    duePerson,
    pay,
    share,
    primaryOf,
    runPrimary,
    actionsOf,
  }
}
