import { type Ref, computed, nextTick, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { kindInfo, useWalletCatalog } from '@/entities/store'

/** The groups of a wallet's settings; each holds one or more sections. */
export type SettingsGroup = 'general' | 'accounts' | 'automations' | 'exchange' | 'danger'

/**
 * Sections that were tabs of their own before the groups: old links
 * (`?tab=deposit`) open the group and scroll to the section.
 */
const SECTION_GROUP: Record<string, SettingsGroup> = {
  property: 'general',
  loyalty: 'general',
  deposit: 'general',
  terms: 'general',
  import: 'exchange',
  syncs: 'exchange',
  imported: 'exchange',
}

/** The id a section carries in the page, for scrolling to it. */
export const sectionId = (section: string) => `wallet-settings-${section}`

/**
 * Which groups a wallet shows (a setting that does not apply is not shown at
 * all), which one is open — kept in the URL — and the jump to a section.
 */
export function useSettingsGroups(store: Ref<Store | undefined>) {
  const route = useRoute()
  const router = useRouter()
  const { t } = useI18n()
  const catalog = useWalletCatalog()
  const isRoot = computed(() => !store.value?.parent_id)

  const groups = computed(() => {
    const current = store.value
    if (!current) return []
    const list: { value: SettingsGroup; label: string }[] = [{ value: 'general', label: t('walletSettings.general') }]
    // Sub-accounts and envelopes make no sense for a kind that takes no sub-accounts (a home, a car).
    if ((kindInfo(catalog.data.value, current.kind)?.sub_types.length ?? 0) > 0) {
      list.push({
        value: 'accounts',
        label: isRoot.value ? t('walletSettings.accounts') : t('walletSettings.envelopes'),
      })
    }
    list.push({ value: 'automations', label: t('walletSettings.automations') })
    if (current.kind === 'exchange' && isRoot.value)
      list.push({ value: 'exchange', label: t('walletSettings.integrations') })
    list.push({ value: 'danger', label: t('walletSettings.danger') })
    return list
  })

  const wanted = computed(() => String(route.query.tab ?? 'general'))
  const group = computed<SettingsGroup>({
    get: () => {
      const target = SECTION_GROUP[wanted.value] ?? wanted.value
      return groups.value.some((entry) => entry.value === target) ? (target as SettingsGroup) : 'general'
    },
    set: (value) => void router.replace({ query: { ...route.query, tab: value } }),
  })

  /** Open the section's group and bring the section into view. */
  function showSection(section: string) {
    void router.replace({ query: { ...route.query, tab: section } })
  }

  // A link to a section scrolls to it once its group has rendered.
  watch(
    [wanted, () => groups.value.length],
    async ([section]) => {
      if (!(section in SECTION_GROUP)) return
      await nextTick()
      const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
      document
        .getElementById(sectionId(section))
        ?.scrollIntoView({ block: 'start', behavior: reduce ? 'auto' : 'smooth' })
    },
    { immediate: true },
  )

  return { groups, group, isRoot, showSection }
}
