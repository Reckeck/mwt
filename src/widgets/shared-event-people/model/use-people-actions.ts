import { type MaybeRefOrGetter, computed, reactive, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pencil, Phone, Trash2, UserPlus, UserRound } from 'lucide-vue-next'

import { guestDraft } from '@/features/shared-event-edit'
import type { GuestDetailsResult } from '@/features/shared-event-edit'
import {
  ME,
  useAddSharedParticipant,
  useMakeSharedParticipantContact,
  useRemoveSharedParticipant,
  useSetSharedOverride,
  useUpdateSharedParticipant,
} from '@/entities/shared-event'
import type { UiAction } from '@/shared/ui/context-menu'

/** What the people table can do with a person and the state of the dialogs those open. */
export function usePeopleActions(view: MaybeRefOrGetter<SharedEventView | undefined>) {
  const { t } = useI18n()
  const details = reactive({ open: false, personId: '' })
  const override = reactive({ open: false, personId: '' })
  const drop = reactive({ open: false, personId: '' })

  const add = useAddSharedParticipant()
  const update = useUpdateSharedParticipant()
  const make = useMakeSharedParticipantContact()
  const remove = useRemoveSharedParticipant()
  const setOverride = useSetSharedOverride()

  const eventId = () => toValue(view)?.event.id ?? ''
  const personOf = (id: string) => toValue(view)?.people.find((person) => person.id === id) ?? null
  const participantOf = (id: string) => toValue(view)?.event.participants.find((person) => person.id === id) ?? null
  const adjustmentOf = (id: string) =>
    toValue(view)?.event.adjustments.find((entry) => entry.participant_id === id) ?? null

  const detailsPerson = computed(() => personOf(details.personId))
  const detailsGuest = computed(() => {
    const person = participantOf(details.personId)
    return person ? { name: person.name, details: person.details } : null
  })
  const overridePerson = computed(() => personOf(override.personId))
  const overrideTotal = computed(() => adjustmentOf(override.personId)?.total ?? '')
  const dropPerson = computed(() => personOf(drop.personId))

  const addContact = (contact: Contact) =>
    add.mutate({
      event_id: eventId(),
      participant: { id: null, name: contact.name, contact_id: contact.id, details: null },
    })
  const addGuest = (name: string) => add.mutate({ event_id: eventId(), participant: guestDraft(name, null) })

  function saveDetails(result: GuestDetailsResult) {
    update.mutate(
      {
        event_id: eventId(),
        participant_id: details.personId,
        participant: guestDraft(result.name, result.details),
      },
      { onSuccess: () => (details.open = false) },
    )
  }

  function makeContact() {
    const person = detailsPerson.value
    if (!person) return
    make.mutate(
      { input: { event_id: eventId(), participant_id: person.id }, name: person.name },
      { onSuccess: () => (details.open = false) },
    )
  }

  function confirmDrop() {
    const person = dropPerson.value
    if (!person) return
    remove.mutate(
      { input: { event_id: eventId(), participant_id: person.id }, name: person.name },
      { onSuccess: () => (drop.open = false) },
    )
  }

  function saveOverride(total: string | null) {
    setOverride.mutate(
      { event_id: eventId(), participant_id: override.personId, total },
      { onSuccess: () => (override.open = false) },
    )
  }

  function actionsOf(person: PersonView): UiAction[] {
    const guest = person.kind === 'guest'
    const fixed = adjustmentOf(person.id) !== null
    return [
      {
        id: 'details',
        label: t('sharedEvents.people.details'),
        icon: Phone,
        hidden: !guest,
        run: () => Object.assign(details, { open: true, personId: person.id }),
      },
      {
        id: 'make-contact',
        label: t('sharedEvents.people.makeContact'),
        icon: UserPlus,
        hidden: !guest,
        run: () => make.mutate({ input: { event_id: eventId(), participant_id: person.id }, name: person.name }),
      },
      {
        id: 'override',
        label: fixed ? t('sharedEvents.people.overrideChange') : t('sharedEvents.people.override'),
        hint: t('sharedEvents.people.overrideHint'),
        icon: Pencil,
        run: () => Object.assign(override, { open: true, personId: person.id }),
      },
      {
        id: 'override-clear',
        label: t('sharedEvents.people.overrideClear'),
        icon: UserRound,
        hidden: !fixed,
        run: () => {
          override.personId = person.id
          saveOverride(null)
        },
      },
      {
        id: 'remove',
        label: t('actions.delete'),
        icon: Trash2,
        destructive: true,
        hidden: person.id === ME,
        run: () => Object.assign(drop, { open: true, personId: person.id }),
      },
    ]
  }

  return {
    details,
    override,
    drop,
    detailsGuest,
    detailsPerson,
    overridePerson,
    overrideTotal,
    dropPerson,
    addContact,
    addGuest,
    saveDetails,
    makeContact,
    confirmDrop,
    saveOverride,
    actionsOf,
    adjustmentOf,
    detailsPending: update.isPending,
    dropPending: remove.isPending,
    overridePending: setOverride.isPending,
  }
}
