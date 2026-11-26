import type { AmountInputProps } from './amount-input.types'

type AmountSize = NonNullable<AmountInputProps['size']>

/** The frame of the field per size. `inline`: no frame of its own, the right half of an exchange box (board 462813). */
export const AMOUNT_BOX_SIZE: Record<AmountSize, string> = {
  default: '',
  lg: 'h-12',
  hero: 'h-16 justify-center border-transparent px-1 focus-within:ring-0',
  box: 'h-16 rounded-xl border-2 border-foreground px-2',
  inline: 'h-10 border-transparent focus-within:ring-0',
}

/** The digits per size. The phone hero: the number and its currency sit together in the middle (board 533927). */
export const AMOUNT_TEXT_SIZE: Record<AmountSize, string> = {
  default: '',
  lg: 'text-xl font-semibold',
  hero: 'w-auto max-w-full flex-none text-center text-hero [field-sizing:content] min-w-[1ch]',
  box: 'text-3xl font-medium',
  inline: 'px-1 text-right text-2xl font-medium',
}

/** The currency after the amount per size. */
export const AMOUNT_SUFFIX_SIZE: Record<AmountSize, string> = {
  default: '',
  lg: '',
  hero: 'pl-1 text-hero text-muted-foreground',
  box: 'pr-3 text-xl text-muted-foreground',
  inline: 'text-sm text-muted-foreground',
}
