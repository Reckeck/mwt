export interface TapHintProps {
  /** What the trigger means; without it the trigger shows plain. */
  hint?: string
  /** A text label with a dotted underline; leave out when the default slot holds an icon. */
  label?: string
  /** Accessible name of an icon trigger (the slot content). */
  ariaLabel?: string
}
