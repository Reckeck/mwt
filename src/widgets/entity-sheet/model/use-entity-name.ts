import { useI18n } from 'vue-i18n'

import { useTxContext } from '@/shared/lib/tx-context'

/**
 * The name of an entity as the core sent it; the core leaves it empty where
 * the frontend names it itself (a built-in category, a limit).
 */
export function useEntityName() {
  const { t } = useI18n()
  const context = useTxContext()
  return (target: EntityRef, label: string): string => {
    if (label) return label
    if (target.kind === 'category') return context.categoryName(target.id)
    return t(`entity.kind.${target.kind}`)
  }
}
