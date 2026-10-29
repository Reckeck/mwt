import { computed, ref } from 'vue'

import { i18n } from '@/shared/config/i18n'
import { nonNegativeAmount, positiveAmount, required } from '@/shared/lib/form'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useStoreTree } from '@/shared/api/queries'
import { useSettingsStore } from '@/shared/store/settings'

/** The fields of a bonus programme, shared by `Add programme` and its settings. */
export function useLoyaltyInfo() {
  const settings = useSettingsStore()
  const tree = useStoreTree()

  const symbol = ref('')
  const value = ref('1')
  const currency = ref(settings.baseCurrency)
  const expiresAt = ref('')
  const expiring = ref('')
  const login = ref('')
  const sources = ref<string[]>([])
  const unit = ref<LoyaltyUnit>('points')
  /** The programme being edited: its own symbol is not taken. */
  const ownId = ref<string | null>(null)

  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
  /** Wallets whose purchases earn the points: any but programmes and property, as the core allows. */
  const cards = computed(() =>
    nodes.value.filter((node) => !node.store.archived && !['loyalty', 'property'].includes(node.store.kind)),
  )
  const takenSymbols = computed(
    () =>
      new Set(
        nodes.value
          .filter((node) => node.store.id !== ownId.value && node.store.loyalty)
          .map((node) => node.store.loyalty?.points_symbol.toUpperCase() ?? ''),
      ),
  )

  function load(store: Store | null) {
    const info = store?.loyalty
    ownId.value = store?.id ?? null
    symbol.value = info?.points_symbol ?? ''
    value.value = info?.point_value ?? '1'
    currency.value = info?.currency ?? settings.baseCurrency
    expiresAt.value = info?.expires_at ?? ''
    expiring.value = info?.expiring_points ?? ''
    login.value = info?.login ?? ''
    sources.value = [...(info?.source_store_ids ?? [])]
    unit.value = info?.unit ?? 'points'
  }

  /** `Tinkoff cashback` → `Chase`, then `TINKOFF2` if another programme has it. */
  function symbolFrom(name: string): string {
    const word = (name.trim().split(/\s+/)[0] ?? '')
      .replace(/[^\p{L}\p{N}]/gu, '')
      .toUpperCase()
      .slice(0, 12)
    const base = word || 'POINTS'
    let candidate = base
    for (let at = 2; takenSymbols.value.has(candidate); at += 1) candidate = `${base.slice(0, 11)}${at}`
    return candidate
  }

  const rules = () => ({
    symbol:
      required(symbol.value) ??
      (takenSymbols.value.has(symbol.value.trim().toUpperCase()) ? i18n.global.t('loyalty.symbolTaken') : null),
    value: positiveAmount(value.value),
    currency: required(currency.value, 'pick'),
    expiring: nonNegativeAmount(expiring.value),
  })

  function toInfo(): LoyaltyInfo {
    return {
      points_symbol: symbol.value.trim(),
      point_value: value.value || '0',
      currency: currency.value,
      expires_at: expiresAt.value || null,
      expiring_points: expiring.value || null,
      login: login.value.trim() || null,
      source_store_ids: sources.value,
      unit: unit.value,
    }
  }

  return { symbol, value, currency, expiresAt, expiring, login, sources, unit, cards, load, rules, symbolFrom, toInfo }
}

export type LoyaltyInfoState = ReturnType<typeof useLoyaltyInfo>
export type LoyaltyField = keyof ReturnType<LoyaltyInfoState['rules']>
