import { type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useIsPhone } from '@/shared/lib/phone'
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

/** What a wallet's page leads with (Wallets-1 · 5–7). */
export type WalletTabsKind = 'home' | 'money' | 'coins' | 'exchange'

/**
 * The tabs a wallet's kind needs, kept in the URL so a link or «Back» returns
 * to them: a card opens on its operations, a coin wallet on its positions, an
 * exchange adds its terminal and keeps the import and relations under «Ещё»;
 * a home opens on its utilities. Old `?view=terminal|import` links still land.
 */
export function useWalletTabs(kind: Ref<WalletTabsKind>, hasTerminal: Ref<boolean>) {
  const { t } = useI18n()
  const route = useRoute()
  const router = useRouter()
  const phone = useIsPhone()

  const label = {
    utilities: () => t('walletSettings.utilities'),
    positions: () => t('pos.title'),
    history: () => t('wallet.tab.operations'),
    relations: () => t('wallet.relations'),
    terminal: () => t('wallet.view.terminal'),
    import: () => t('wallet.view.import'),
  }
  const tabOf = (value: keyof typeof label) => ({ value, label: label[value]() })

  const tabs = computed(() => {
    if (kind.value === 'home') return (['utilities', 'positions', 'history', 'relations'] as const).map(tabOf)
    if (kind.value === 'money') return [tabOf('history'), tabOf('positions'), tabOf('relations')]
    if (kind.value === 'coins') return [tabOf('positions'), tabOf('history'), tabOf('relations')]
    return [tabOf('positions'), tabOf('history'), ...(hasTerminal.value && !phone.value ? [tabOf('terminal')] : [])]
  })
  /** A phone has no room for the terminal in the strip (Wallets-1 · 7): it joins «Ещё». */
  const more = computed(() =>
    kind.value === 'exchange'
      ? [...(hasTerminal.value && phone.value ? [tabOf('terminal')] : []), tabOf('import'), tabOf('relations')]
      : [],
  )

  const known = (value: unknown) => [...tabs.value, ...more.value].some((tab) => tab.value === value)
  const asked = () => route.query.tab ?? route.query.view
  const tab = computed({
    get: () => (known(asked()) ? String(asked()) : (tabs.value[0]?.value ?? 'positions')),
    set: (value: string) => void router.replace({ query: { ...route.query, tab: value, view: undefined } }),
  })

  return { tab, tabs, more }
}
