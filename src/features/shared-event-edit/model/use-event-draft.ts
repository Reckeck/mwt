import { type Ref, computed, ref, watch } from 'vue'

import { useI18n } from 'vue-i18n'

import { useSettingsStore } from '@/shared/store/settings'
import type { DateRange } from '@/shared/lib/date-range'
import { isRangeReversed } from '@/shared/lib/date-range'
import { required, useFormValidation } from '@/shared/lib/form'

import { DEFAULT_ROUNDING_STEP } from '../shared-event-edit.constants'
import type { DraftPerson } from '../shared-event-edit.types'

import { guestDraft } from './guest-details'

/**
 * The fields of `New event` / `Edit event` and what the save sends. The people are
 * a draft only while the event is new; later they are changed one by one on the event screen.
 */
export function useEventDraft(open: Ref<boolean>, event: () => SharedEvent | null | undefined) {
  const { t } = useI18n()
  const settings = useSettingsStore()

  const title = ref('')
  const multiDay = ref(false)
  const dates = ref<DateRange | null>(null)
  const currency = ref('')
  const step = ref(DEFAULT_ROUNDING_STEP)
  const category = ref('')
  const organizerIsMe = ref(true)
  const organizerId = ref('')
  const iParticipate = ref(true)
  const people = ref<DraftPerson[]>([])

  watch(open, (now) => {
    if (!now) return
    const current = event()
    title.value = current?.title ?? ''
    multiDay.value = Boolean(current?.ends_on)
    dates.value = current?.starts_on ? { start: current.starts_on, end: current.ends_on } : null
    currency.value = (current?.currency ?? settings.baseCurrency).toLowerCase()
    step.value = current?.rounding_step ?? DEFAULT_ROUNDING_STEP
    category.value = current?.category_id ?? ''
    organizerIsMe.value = !current || current.organizer === 'me'
    organizerId.value = current && current.organizer !== 'me' ? current.organizer : ''
    iParticipate.value = current?.i_participate ?? true
    people.value = []
  })

  /** The people the organizer can be picked from: the draft, or the event's own. */
  const candidates = computed(() => {
    const current = event()
    if (current) return current.participants.map((person) => ({ id: person.id, name: person.name }))
    return people.value.map((person) => ({ id: person.id, name: person.name }))
  })

  const form = useFormValidation(
    () => ({
      title: required(title.value),
      dates:
        multiDay.value && dates.value && isRangeReversed(dates.value) ? t('sharedEvents.edit.datesReversed') : null,
      currency: required(currency.value, 'pick'),
      organizer: organizerIsMe.value || organizerId.value ? null : t('sharedEvents.edit.organizerRequired'),
    }),
    { resetOn: open },
  )

  function addContact(contact: Contact) {
    if (people.value.some((person) => person.contactId === contact.id)) return
    people.value = [
      ...people.value,
      { id: crypto.randomUUID(), name: contact.name, contactId: contact.id, details: null },
    ]
  }

  function addGuest(name: string) {
    people.value = [...people.value, { id: crypto.randomUUID(), name, contactId: null, details: null }]
  }

  function removePerson(id: string) {
    people.value = people.value.filter((person) => person.id !== id)
    if (organizerId.value === id) organizerId.value = ''
  }

  function updateGuest(id: string, name: string, details: GuestDetails) {
    people.value = people.value.map((person) => (person.id === id ? { ...person, name, details } : person))
  }

  /** The request: one day keeps only the start date; the draft people go with a new event. */
  function toDraft(): SharedEventDraft {
    const range = dates.value
    return {
      title: title.value.trim(),
      note: event()?.note ?? '',
      starts_on: range?.start || null,
      ends_on: multiDay.value && range?.end ? range.end : null,
      currency: currency.value,
      rounding_step: step.value,
      category_id: category.value || null,
      organizer: organizerIsMe.value ? 'me' : organizerId.value,
      i_participate: iParticipate.value,
      participants: event()
        ? []
        : people.value.map((person) =>
            person.contactId
              ? { id: person.id, name: person.name, contact_id: person.contactId, details: null }
              : guestDraft(person.name, person.details, person.id),
          ),
    }
  }

  return {
    title,
    multiDay,
    dates,
    currency,
    step,
    category,
    organizerIsMe,
    organizerId,
    iParticipate,
    people,
    candidates,
    form,
    addContact,
    addGuest,
    removePerson,
    updateGuest,
    toDraft,
  }
}
