export interface DropdownMenuItemProps {
  /** Emitted value when the row is chosen. */
  value: string
  destructive?: boolean
  /** Shown but not selectable; pair it with a `title` saying why. */
  disabled?: boolean
}
