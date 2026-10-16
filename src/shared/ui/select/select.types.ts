export interface SelectProps {
  /** Shown while nothing is picked (and no `SelectItem value=""` stands for «nothing»). */
  placeholder?: string
  disabled?: boolean
  /** Red border and `aria-invalid`, for a field with an error under it. */
  invalid?: boolean
  /** Width of the list: `trigger` (default) matches the field, `auto` fits the longest item. */
  contentWidth?: 'trigger' | 'auto'
}

export interface SelectItemProps {
  value: string
  disabled?: boolean
}
