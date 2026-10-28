import { computed, ref, toRaw, watch } from 'vue'

import { parseDecimal } from '@/shared/lib/decimal'
import { useSettingsStore } from '@/shared/store/settings'

import { debtChoices, suggestedProperty } from '../transaction-form.model'
import { useAccountAsset } from './use-account-asset'
import type { TxFormState } from './use-tx-form-state'

/**
 * Values the form fills in by itself — and never over what the user typed:
 * the asset a wallet or a side starts with, the received amount from the
 * rate, the network of the asset; plus the side swap and the debt choice.
 */
/** Quote coins a spot trade spends by default, in order of preference. */
const SPOT_QUOTES = ['USDT', 'USDC', 'FDUSD', 'BUSD']

export function useTxAutofill(state: TxFormState) {
  const settings = useSettingsStore()
  const { props, form, editing, isProperty, storeKind, overview, contextStore, properties } = state

  /** Rent goes to the one flat, fuel to the one car — unless the user picked a property already. */
  let autoProperty = ''
  watch(
    () => [form.categories.join(','), properties.value.length],
    () => {
      if (editing.value || (form.propertyId && form.propertyId !== autoProperty)) return
      autoProperty = suggestedProperty(
        form.categories,
        properties.value.map((node) => node.store),
      )
      form.propertyId = autoProperty
    },
    { immediate: true },
  )

  /** Property is named, not searched: a flat is not on CoinGecko. */
  const propertyName = ref(form.asset && isProperty.value ? form.asset.name : '')
  watch(propertyName, (name) => {
    if (!isProperty.value || form.kind !== 'topup') return
    const symbol = name.trim().toUpperCase().replace(/\s+/g, '_').slice(0, 24)
    form.asset = symbol
      ? { symbol, name: name.trim(), coingecko_id: null, chain: null, contract: null, quote: symbol }
      : null
  })

  /** Swap the ends of a transfer or conversion. */
  function swapSides() {
    const from = form.from
    form.from = form.to
    form.to = from
    if (form.kind === 'convert') {
      const asset = form.fromAsset
      form.fromAsset = form.toAsset
      form.toAsset = asset
      const amount = form.fromAmount
      form.fromAmount = form.toAmount
      form.toAmount = amount
    }
  }

  /* --- the asset a side starts with, so it is not picked twice --- */
  const fromAccount = useAccountAsset(computed(() => form.from))
  const toAccount = useAccountAsset(computed(() => form.to))
  /** What the form set by itself; a value the user picked is never replaced. */
  const autoSet: Partial<Record<'fromAsset' | 'toAsset' | 'asset' | 'soldAsset', AssetRef>> = {}
  function autoFill(key: 'fromAsset' | 'toAsset' | 'asset' | 'soldAsset', value: AssetRef | null) {
    if (editing.value || !value) return
    const current = form[key]
    // `form` is reactive: what it hands back is a proxy of what was stored.
    if (current && toRaw(current) !== autoSet[key]) return
    autoSet[key] = toRaw(value)
    form[key] = value
  }
  /**
   * A new operation starts with the wallet's main asset (its largest position);
   * an empty bank or cash wallet starts with the base currency. Nothing is
   * guessed before the wallet's positions and the app settings are in: an
   * early guess (USD) would then hold, as an autofill is replaced only by
   * another autofill.
   */
  watch(
    () => [overview.data.value, overview.isSuccess.value, storeKind.value, settings.appLoaded, form.kind] as const,
    ([data, loaded, kind, settingsLoaded, formKind]) => {
      if (editing.value || !loaded || !kind || kind === 'property') return
      // A wallet bound to an asset («Карта» in dollars) starts with it, before whatever it holds.
      const binding = contextStore.value?.binding
      const bound = binding && typeof binding === 'object' && 'asset' in binding ? binding.asset : null
      // Staking and pools start with the dollar stablecoin held, like a spot trade, not whatever is listed first.
      const stable =
        formKind === 'defi'
          ? data?.positions.find((position) => SPOT_QUOTES.includes(position.asset.symbol.toUpperCase()))?.asset
          : undefined
      const main = bound ?? stable ?? data?.positions[0]?.asset
      if (main) autoFill('asset', main)
      else if ((kind === 'bank' || kind === 'cash') && settingsLoaded) {
        const code = settings.baseCurrency.toUpperCase()
        autoFill('asset', { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null, quote: code })
      }
    },
    { immediate: true },
  )

  // The account money leaves gives its first asset.
  watch(
    () => [fromAccount.main.value, form.kind] as const,
    ([main, kind]) => {
      if (kind === 'convert') autoFill('fromAsset', main)
      if (kind === 'transfer') autoFill('asset', main)
    },
    { immediate: true },
  )
  // Money arriving on a bank or cash account arrives in its currency.
  watch(
    () => [toAccount.currency.value, form.kind] as const,
    ([currency, kind]) => {
      if (kind === 'convert' && toAccount.moneyOnly.value) autoFill('toAsset', currency)
    },
    { immediate: true },
  )
  // A spot trade usually spends the quote: the pair's quote, else a dollar stablecoin held, else the largest position.
  const spotSold = computed<AssetRef | null>(() => {
    const binding = contextStore.value?.binding
    if (binding && typeof binding === 'object' && 'pair' in binding) return binding.pair.quote
    const positions = overview.data.value?.positions ?? []
    const stable = positions.find((position) => SPOT_QUOTES.includes(position.asset.symbol.toUpperCase()))
    return (stable ?? positions[0])?.asset ?? null
  })
  watch(
    () => [spotSold.value, form.kind] as const,
    ([sold, kind]) => {
      if (kind === 'trade_spot') autoFill('soldAsset', sold)
    },
    { immediate: true },
  )

  /* --- the rate and the received amount follow each other --- */
  let autoToAmount = ''
  watch(
    () => [form.fromAmount, form.rate] as const,
    ([give, rate]) => {
      if (form.kind !== 'convert' || editing.value) return
      const amount = Number(parseDecimal(give) ?? Number.NaN)
      const price = Number(parseDecimal(rate) ?? Number.NaN)
      if (!(amount > 0 && price > 0)) return
      // With several receiving accounts each says its own amount.
      if (form.extra.length || (form.toAmount.trim() && form.toAmount !== autoToAmount)) return
      autoToAmount = String(Number((amount * price).toFixed(8)))
      form.toAmount = autoToAmount
    },
  )
  /** Everything the exchange brings in, over all receiving accounts. */
  const receivedTotal = computed(() =>
    [form.toAmount, ...form.extra.map((row) => row.receive)].reduce(
      (sum, value) => sum + Number(parseDecimal(value) ?? 0),
      0,
    ),
  )
  /** «1 USDT = 92.5 RUB» from the two amounts, when no rate was typed. */
  const impliedRate = computed(() => {
    if (form.kind !== 'convert' || form.rate.trim()) return ''
    const give = Number(parseDecimal(form.fromAmount) ?? Number.NaN)
    const got = receivedTotal.value
    if (!(give > 0 && got > 0) || !form.fromAsset || !form.toAsset) return ''
    const rate = got / give
    return `1 ${form.fromAsset.symbol} = ${Number(rate.toPrecision(8))} ${form.toAsset.symbol}`
  })

  /* --- P2P: the money goes to a person and comes back from them --- */
  function pickConvertMode(value: unknown) {
    if (value !== 'direct' && value !== 'p2p') return
    form.convertMode = value
    // Through someone, both ends are your own accounts.
    if (value === 'p2p' && form.to && !('store' in form.to)) form.to = null
  }

  // The first account an exchange lands on is there to pick, not picked for you.
  watch(
    () => form.kind,
    (kind) => {
      if (kind === 'transfer' && form.to && 'store' in form.to && form.to.store === props.storeId) form.to = null
    },
    { immediate: true },
  )

  const debts = computed(() => debtChoices(form))
  /** The select holds a string; `''` is "not a debt". */
  const debtValue = computed({
    get: () => form.debt ?? '',
    set: (value: string) => {
      form.debt = (value || null) as DebtEffect | null
    },
  })
  watch(debts, (choices) => {
    if (form.debt && !choices.includes(form.debt)) form.debt = null
  })

  // The network follows the picked asset.
  watch(
    () => form.asset,
    (asset) => {
      if (asset?.chain && asset.chain !== 'fiat') form.network = asset.chain
    },
  )

  return { propertyName, swapSides, toAccount, receivedTotal, impliedRate, pickConvertMode, debts, debtValue }
}
