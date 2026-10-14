/**
 * What the picker holds, as the string a native input would:
 * `date` → `2026-10-14`, `datetime` → `2026-10-14T09:30`, `month` → `2026-10`.
 */
export type DateTimePickerMode = 'date' | 'datetime' | 'month'

export interface DateTimePickerProps {
  mode?: DateTimePickerMode
  /** Shown while empty. */
  placeholder?: string
  /** No «Очистить»: the field always has a value. */
  required?: boolean
  /** Red border, as for a field with an error under it. */
  invalid?: boolean
  disabled?: boolean
  /** Earliest and latest allowed value, in the same format as the model. */
  min?: string
  max?: string
}
