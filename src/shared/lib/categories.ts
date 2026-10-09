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
