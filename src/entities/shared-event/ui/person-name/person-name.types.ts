export interface PersonNameProps {
  /** The name; empty reads as `Me`. */
  name: string
  kind: PersonKind
  /** A guest with a phone, messenger, requisites or a note: a phone icon after the name. */
  hasDetails?: boolean
  /** A contact made by name only, so far: the `quick` label. */
  quick?: boolean
}
