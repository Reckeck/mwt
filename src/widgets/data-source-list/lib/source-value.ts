/** The value a source holds now, as text: a number, the last point of a series or the text. */
export function sourceValueText(value: SourceValue | null): string | null {
  if (!value) return null
  if (value.number !== null) return value.number
  const last = value.series.at(-1)
  if (last) return last.value
  return value.text
}
