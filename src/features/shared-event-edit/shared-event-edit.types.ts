/** A person typed into the dialog before the event exists: a contact, or a guest by name. */
export interface DraftPerson {
  /** Made here, so the dialog can name the person as the organizer in the same request. */
  id: string
  name: string
  contactId: string | null
  details: GuestDetails | null
}

export interface SharedEventEditDialogProps {
  /** The event to change; none creates a new one. */
  event?: SharedEvent | null
}

export interface GuestDetailsDialogProps {
  /** The guest's name and details now; `null` while closed. */
  guest: { name: string; details: GuestDetails | null } | null
  /** «Гость события «Шашлыки · 2–4 ноя»» under the name. */
  eventLabel: string
  /** «Сделать контактом» is offered only for a guest that is already in an event. */
  canMakeContact?: boolean
  pending?: boolean
}

export interface GuestDetailsResult {
  name: string
  details: GuestDetails
}
