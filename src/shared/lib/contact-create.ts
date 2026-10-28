import { type InjectionKey, inject, provide } from 'vue'

/**
 * Creates a contact on the spot and answers with it, or `null` when the form was
 * closed unsaved. The form lives in a feature, the pickers in lower layers: the shell
 * hands the function down once (like `tx-glances`), and every contact picker uses it.
 */
export type ContactCreator = (name: string) => Promise<Contact | null>

export const CONTACT_CREATOR_KEY: InjectionKey<ContactCreator> = Symbol('contact-creator')

export function provideContactCreator(creator: ContactCreator): void {
  provide(CONTACT_CREATOR_KEY, creator)
}

/** `null` where no shell provides one (a picker then only picks). */
export function useContactCreator(): ContactCreator | null {
  return inject(CONTACT_CREATOR_KEY, null)
}
