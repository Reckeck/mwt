import { type Ref, computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { useStorage } from '@vueuse/core'

/** With sub-accounts or only the wallet itself — remembered per wallet. */
export function useWalletScope(storeId: Ref<string>) {
  // Stored as '1' for «only this wallet», nothing for the default — as before.
  const onlyOwn = useStorage(`wallet-only-own:${storeId.value}`, false, undefined, {
    serializer: { read: (raw) => raw === '1', write: (value) => (value ? '1' : '') },
  })
  const includeChildren = computed(() => !onlyOwn.value)
  const scopeValue = computed({
    get: () => (onlyOwn.value ? 'own' : 'all'),
    set: (value: string) => {
      if (value) onlyOwn.value = value === 'own'
    },
  })
  return { includeChildren, scopeValue }
}

/**
 * The exchange view and the tab, kept in the URL so a link or «Back» returns
 * to them; a home opens on its utilities, everything else on positions.
 */
export function useWalletTabs(isPropertyRoot: Ref<boolean>, hasTerminal: Ref<boolean>) {
  const { t } = useI18n()
  const route = useRoute()
  const router = useRouter()
  const setQuery = (key: string, value: string) => void router.replace({ query: { ...route.query, [key]: value } })

  const defaultTab = computed(() => (isPropertyRoot.value ? 'utilities' : 'positions'))
  const tab = ref(String(route.query.tab ?? defaultTab.value))
  watch(defaultTab, (value) => {
    if (!route.query.tab) tab.value = value
  })
  watch(tab, (value) => {
    if (route.query.tab !== value) setQuery('tab', value)
  })
  const tabs = computed(() => [
    ...(isPropertyRoot.value ? [{ value: 'utilities', label: t('walletSettings.utilities') }] : []),
    { value: 'positions', label: t('pos.title') },
    { value: 'history', label: t('hist.title') },
    { value: 'relations', label: t('wallet.relations') },
  ])

  /* --- an exchange page has views: overview, terminal, import --- */
  const views = computed(() => [
    { value: 'overview', label: t('wallet.view.overview') },
    { value: 'terminal', label: t('wallet.view.terminal') },
    { value: 'import', label: t('wallet.view.import') },
  ])
  const view = computed({
    get: () => (hasTerminal.value ? String(route.query.view ?? 'overview') : 'overview'),
    set: (value: string) => setQuery('view', value),
  })

  return { tab, tabs, view, views }
}
