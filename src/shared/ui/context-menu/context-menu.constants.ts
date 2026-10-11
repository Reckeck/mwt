/** Item rows shared by the context menu and the dropdown, so both look the same. */
export const MENU_ITEM_CLASS =
  'group/item flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm outline-none data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground'

/** Dangerous items: red text and icon, a red-tinted highlight. */
export const MENU_ITEM_DESTRUCTIVE_CLASS =
  'text-destructive data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive [&_svg]:text-destructive'

/** Ordinary items keep their icon muted, so a red icon stands out. */
export const MENU_ITEM_ICON_CLASS = 'size-4 shrink-0 text-muted-foreground group-data-[highlighted]/item:text-current'

export const MENU_CONTENT_CLASS =
  'z-50 min-w-48 overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-md'

/** Visible actions, with separators only between non-empty groups. */
export function visibleActions<T extends { hidden?: boolean }>(actions: T[]): T[] {
  return actions.filter((action) => !action.hidden)
}
