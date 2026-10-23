/** Minimal shape of vue-i18n's `t`. */
type Translate = (key: string) => string

/** A category's label: the user's name, else the built-in translation of its key. */
export function categoryLabel(category: Pick<Category, 'key' | 'name'>, t: Translate): string {
  if (category.name) return category.name
  const key = `categories.default.${category.key}`
  const known = t(key)
  return known === key ? category.key : known
}

/** Label for a bare key, looked up in a category list. */
export function categoryKeyLabel(key: string, categories: Category[] | undefined, t: Translate): string {
  const found = categories?.find((category) => category.key === key)
  return categoryLabel(found ?? { key, name: null }, t)
}

export const CATEGORY_GROUPS: CategoryGroup[] = ['income', 'fixed', 'daily', 'savings', 'service']

/** Everyday categories that count as a must until the user says otherwise; mirrors `DEFAULT_ESSENTIAL` in core. */
export const DEFAULT_ESSENTIAL: readonly string[] = [
  'groceries',
  'transport',
  'fuel',
  'medicine',
  'pharmacy',
  'household_chemicals',
  'pets',
  'therapy',
  'education',
  'repair',
  'family_help',
]

/** Whether spending in the category is mandatory: fixed always, everyday by flag or default. */
export function isEssential(category: Pick<Category, 'key' | 'group' | 'essential'>): boolean {
  if (category.group === 'fixed') return true
  if (category.group !== 'daily') return false
  return category.essential ?? DEFAULT_ESSENTIAL.includes(category.key)
}

/** Income from capital until the user says otherwise; mirrors `DEFAULT_PASSIVE` in core. */
export const DEFAULT_PASSIVE: readonly string[] = [
  'deposit_interest',
  'earn',
  'dividends',
  'coupons',
  'staking',
  'rent_income',
]

/** Whether income in the category is passive (from capital): income only, by flag or default. */
export function isPassive(category: Pick<Category, 'key' | 'group' | 'passive'>): boolean {
  if (category.group !== 'income') return false
  return category.passive ?? DEFAULT_PASSIVE.includes(category.key)
}

/** Live categories of the given groups as picker options, grouped under the group's name. */
export function categoryOptions(
  categories: Category[] | undefined,
  t: Translate,
  groups: readonly CategoryGroup[] = ['daily', 'fixed', 'income', 'savings'],
): { value: string; label: string; group: string; groupLabel: string }[] {
  return groups.flatMap((group) =>
    (categories ?? [])
      .filter((category) => category.group === group && !category.archived)
      .map((category) => ({
        value: category.key,
        label: categoryLabel(category, t),
        group,
        groupLabel: t(`categoryGroup.${group}`),
      })),
  )
}
