/** The minimum of a route record the tab strip reads. */
export interface TabRecord {
  name?: string | symbol
  meta: { group?: string }
}

export interface BudgetTab {
  name: string
  /** i18n key of the label. */
  labelKey: string
}

/**
 * The tabs of the Budget section: one per child route of the section frame, in route order. The routes
 * decide the set (the router gives the frame only the Budget routes), so there is no second list here.
 */
export function budgetTabs(children: readonly TabRecord[]): BudgetTab[] {
  return children.flatMap((child) =>
    typeof child.name === 'string' && child.meta.group
      ? [{ name: child.name, labelKey: `money.tab.${child.meta.group}` }]
      : [],
  )
}
