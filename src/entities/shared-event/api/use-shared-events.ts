import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { sharedEventKeys } from './shared-event-keys'

/** One event with its people, items and settlements (`get_shared_event`). */
export function useSharedEvent(id: MaybeRefOrGetter<string>, enabled: MaybeRefOrGetter<boolean> = true) {
  return useQuery({
    // no placeholder: another event's people must not show under this one's title
    queryKey: computed(() => sharedEventKeys.detail(toValue(id))),
    queryFn: () => call<SharedEventView>('get_shared_event', { id: toValue(id) }),
    enabled: computed(() => toValue(enabled)),
  })
}

/** The list of events, newest first; archived ones only on request. */
export function useSharedEvents(includeArchived: MaybeRefOrGetter<boolean> = false) {
  return useQuery({
    queryKey: computed(() => sharedEventKeys.list(toValue(includeArchived))),
    queryFn: () =>
      call<SharedEventSummary[]>('list_shared_events', { filter: { include_archived: toValue(includeArchived) } }),
    placeholderData: keepPreviousData,
  })
}

/** What the contact and I share through events (`get_contact_shared_events`); positive: they owe me. */
export function useContactSharedEvents(contactId: MaybeRefOrGetter<string>) {
  return useQuery({
    // no placeholder: another contact's events
    queryKey: computed(() => sharedEventKeys.contact(toValue(contactId))),
    queryFn: () => call<ContactEventBalance[]>('get_contact_shared_events', { contactId: toValue(contactId) }),
  })
}

/** The text for a chat: purchases, shares, who pays whom, the organizer's requisites. */
export function useSharedEventShareText(
  eventId: MaybeRefOrGetter<string>,
  locale: MaybeRefOrGetter<string>,
  enabled: MaybeRefOrGetter<boolean>,
) {
  return useQuery({
    // no placeholder: another event's text
    queryKey: computed(() => sharedEventKeys.shareText(toValue(eventId), toValue(locale))),
    queryFn: () =>
      call<string>('get_shared_event_share_text', { input: { event_id: toValue(eventId), locale: toValue(locale) } }),
    enabled: computed(() => toValue(enabled)),
    // The text follows the debts, which change under it: never show an old one.
    staleTime: 0,
    gcTime: 0,
  })
}
