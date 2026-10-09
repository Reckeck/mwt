/** Panel alignment against the trigger edge. */
export const dropdownMenuAligns = {
  start: 'start',
  center: 'center',
  end: 'end',
} as const

export type DropdownMenuAlign = keyof typeof dropdownMenuAligns

export interface DropdownMenuProps {
  /** Align the panel against the trigger edge. */
  align?: DropdownMenuAlign
}
