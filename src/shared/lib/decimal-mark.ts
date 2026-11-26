import { numberFormat } from './format'

/** The decimal mark of the UI language (`,` in Russian), for fields and keypads that type numbers. */
export function decimalMark(): string {
  return (
    numberFormat()
      .formatToParts(1.5)
      .find((part) => part.type === 'decimal')?.value ?? '.'
  )
}
