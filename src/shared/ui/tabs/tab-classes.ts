/** One pill look for `Tabs` and `RouteTabs`: a muted pill track, the open tab a raised pill. */
export const TAB_LIST_CLASS =
  'scrollbar-none inline-flex max-w-full items-center gap-0.5 overflow-x-auto rounded-full bg-muted p-[3px]'

export const TAB_CLASS = [
  'flex h-[1.875rem] shrink-0 items-center gap-1 whitespace-nowrap rounded-full px-3 text-sm font-medium',
  'text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
  'data-[state=active]:bg-background data-[state=active]:text-foreground',
].join(' ')
