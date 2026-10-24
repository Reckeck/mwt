/** Groups a user's category can belong to: the service group is the app's own. */
export const EDITABLE_GROUPS: CategoryGroup[] = ['daily', 'fixed', 'income', 'savings']

/** The tab's blocks, in order: «Расходы» holds two groups, the rest one each. */
export const SECTIONS: { id: string; groups: CategoryGroup[]; title: string }[] = [
  { id: 'spend', groups: ['daily', 'fixed'], title: 'categories.manage.sectionSpend' },
  { id: 'income', groups: ['income'], title: 'categoryGroup.income' },
  { id: 'savings', groups: ['savings'], title: 'categoryGroup.savings' },
]
