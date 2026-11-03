import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { categoryLabel } from '@/shared/lib/categories'

/** The category mutations of the «Categories» tab; every write refreshes the list. */
export function useCategoryActions() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ['categories'] })

  const update = useMutation({
    mutationFn: (input: UpdateCategoryInput) => call<Category>('update_category', { input }),
    onSuccess: refresh,
    meta: { success: 'categories.manage.renamed', error: 'settings.saveFailed' },
  })

  /** The essential and passive switches: the toast names the flag that changed. */
  const flag = useMutation({
    mutationFn: (input: UpdateCategoryInput) => call<Category>('update_category', { input }),
    onSuccess: refresh,
    meta: {
      error: 'settings.saveFailed',
      success: (_data, input) => {
        const { passive, essential } = input as UpdateCategoryInput
        if (passive !== null) return passive ? t('categories.manage.passiveOn') : t('categories.manage.passiveOff')
        return essential ? t('categories.manage.essentialOn') : t('categories.manage.essentialOff')
      },
    },
  })

  const archive = useMutation({
    mutationFn: (category: Category) => call('archive_category', { key: category.key, archived: !category.archived }),
    onSuccess: refresh,
    meta: {
      error: 'settings.saveFailed',
      success: (_data, category) =>
        (category as Category).archived ? t('categories.manage.restored') : t('categories.manage.archived'),
    },
  })

  const remove = useMutation({
    mutationFn: (key: string) => call('delete_category', { key }),
    onSuccess: refresh,
    meta: { success: 'categories.manage.deleted', error: 'categories.manage.deleteFailed' },
  })

  const create = useMutation({
    mutationFn: (input: { name: string; group: CategoryGroup }) =>
      call<Category>('create_category', { input: { key: null, name: input.name.trim(), group: input.group } }),
    onSuccess: refresh,
    meta: { success: 'categories.manage.created', error: 'settings.saveFailed' },
  })

  /**
   * Each change saves at once, so the category as the row showed it is the `base`: a sync that
   * changed another field of it in the meantime is kept.
   * Rename on blur; an empty name returns to the built-in label. */
  function rename(category: Category, raw: string) {
    const name = raw.trim()
    if (name === categoryLabel(category, t)) return
    update.mutate({
      key: category.key,
      name: name || null,
      group: category.group,
      essential: null,
      passive: null,
      base: category,
    })
  }
  const regroup = (category: Category, group: CategoryGroup) =>
    update.mutate({ key: category.key, name: category.name, group, essential: null, passive: null, base: category })
  const setFlag = (category: Category, on: boolean) =>
    flag.mutate({
      key: category.key,
      name: category.name,
      group: category.group,
      essential: category.group === 'daily' ? on : null,
      passive: category.group === 'income' ? on : null,
      base: category,
    })

  return { update, flag, archive, remove, create, rename, regroup, setFlag }
}

export type CategoryActions = ReturnType<typeof useCategoryActions>
