import { EMPTY_DETAILS } from '../shared-event-edit.constants'

/** Anything filled in: the phone icon on the row and the details sent to the backend depend on it. */
export function hasGuestDetails(details: GuestDetails | null | undefined): boolean {
  if (!details) return false
  return Boolean(details.phone.trim() || details.messenger.trim() || details.note.trim() || details.requisites.length)
}

/** The request for a guest: a name and trimmed details (none at all when nothing is filled in). */
export function guestDraft(
  name: string,
  details: GuestDetails | null,
  id: string | null = null,
): SharedParticipantDraft {
  const filled = hasGuestDetails(details)
  return {
    id,
    name: name.trim(),
    contact_id: null,
    details: filled
      ? {
          ...EMPTY_DETAILS,
          ...details,
          phone: details?.phone.trim() ?? '',
          messenger: details?.messenger.trim() ?? '',
          note: details?.note.trim() ?? '',
        }
      : null,
  }
}
