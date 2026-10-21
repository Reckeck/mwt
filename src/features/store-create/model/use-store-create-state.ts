import { type InjectionKey, type Ref, computed, inject, provide, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { emptyLiabilityTerms, kindInfo, parsePairSymbol, useWalletCatalog } from '@/entities/store'
import { fiatRef, tickerRef } from '@/shared/lib/asset-ref'
import { required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import { DEFAULT_STORE_COLOR, KIND_COLORS } from '../store-create.constants'
import type { StoreCreateDialogProps } from '../store-create.types'
import { useStoreTree } from '@/shared/api/queries'

/**
 * Everything the new-wallet dialog holds: the kind or sub-account type picked
 * on step 1, the fields step 2 shows for it, and their checks. Every opening
 * starts clean.
 */
function setup(props: StoreCreateDialogProps, open: Ref<boolean>) {
  const { t } = useI18n()
  const settings = useSettingsStore()
  const catalog = useWalletCatalog()

  const isSub = computed(() => Boolean(props.parent))

  /* --- step 1: what --- */
  const step = ref<'pick' | 'details'>('pick')
  const kind = ref<StoreKind>('bank')
  const subType = ref<SubType | null>(null)
  const creatableKinds = computed(() =>
    (catalog.data.value?.kinds ?? []).filter((info) => info.user_creatable && info.kind !== 'loyalty'),
  )
  const subTypes = computed(() =>
    props.parent ? (kindInfo(catalog.data.value, props.parent.kind)?.sub_types ?? []) : [],
  )
  const subInfo = computed(() => subTypes.value.find((info) => info.sub_type === subType.value) ?? null)

  /* --- step 2: details --- */
  const name = ref('')
  const nameTouched = ref(false)
  const color = ref(DEFAULT_STORE_COLOR)
  const terms = ref<LiabilityTerms>(emptyLiabilityTerms())
  const custody = ref<Custody>('hot')
  const provider = ref<ExchangeProvider | null>(null)
  /** Optional read-only Bybit key: with it the exchange gets its accounts right away. */
  const apiKey = ref('')
  const apiSecret = ref('')
  const withKey = computed(
    () => provider.value === 'bybit' && apiKey.value.trim() !== '' && apiSecret.value.trim() !== '',
  )
  const propertyCategory = ref<PropertyCategory>('flat')
  const securedBy = ref('')
  const currency = ref('')
  const token = ref<AssetRef | null>(null)
  /** A new bank comes with its first card: a card is a bank account. */
  const cardCurrency = ref('')
  const cardName = ref('')
  const withCard = computed(() => !isSub.value && kind.value === 'bank')
  const pairText = ref('')
  const pair = computed(() => parsePairSymbol(pairText.value))

  /** What the sub-account is locked to, from the fields of its shape. */
  const binding = computed<Binding | null>(() => {
    const info = subInfo.value
    if (!info) return null
    if (info.shape === 'market') return null // the backend fills the market in
    if (info.shape === 'pair') {
      return pair.value ? { pair: { base: tickerRef(pair.value.base), quote: tickerRef(pair.value.quote) } } : null
    }
    if (props.parent?.kind === 'crypto') return token.value ? { asset: token.value } : null
    return currency.value ? { asset: fiatRef(currency.value) } : null
  })

  /* --- optional opening balance: what the wallet already holds --- */
  const openingAmount = ref('')
  const openingCurrency = ref('')
  const openingToken = ref<AssetRef | null>(null)
  const openingSide = ref<'base' | 'quote'>('quote')
  /** Property is valued by purchase and a loan by its terms: no opening balance there. */
  const offersOpening = computed(() => !['property', 'liability'].includes(kind.value))
  /** Which asset the opening balance is in, from the binding or the picker. */
  const openingAsset = computed<AssetRef | null>(() => {
    const current = binding.value
    if (current && typeof current === 'object' && 'asset' in current) return current.asset
    if (current && typeof current === 'object' && 'pair' in current) return current.pair[openingSide.value]
    if (withCard.value) return cardCurrency.value ? fiatRef(cardCurrency.value) : null
    if (kind.value === 'bank' || kind.value === 'cash')
      return openingCurrency.value ? fiatRef(openingCurrency.value) : null
    return openingToken.value
  })
  /** The binding fixes the asset; otherwise the user picks it. */
  const openingPicker = computed<'none' | 'currency' | 'asset' | 'side'>(() => {
    const current = binding.value
    if (current && typeof current === 'object' && 'asset' in current) return 'none'
    if (current && typeof current === 'object' && 'pair' in current) return 'side'
    if (withCard.value) return 'none'
    return kind.value === 'bank' || kind.value === 'cash' ? 'currency' : 'asset'
  })
  const openingAmountValue = computed(() => openingAmount.value.trim().replace(',', '.'))
  const openingValid = computed(
    () => !openingAmountValue.value || (Number(openingAmountValue.value) > 0 && openingAsset.value !== null),
  )

  const properties = useStoreTree(computed(() => open.value && kind.value === 'liability'))
  const propertyRoots = computed(() =>
    (properties.data.value ?? []).filter((node) => node.store.kind === 'property' && !node.store.archived),
  )

  /** A sub-account name follows its type and binding until the user types one. */
  const suggestedName = computed(() => {
    if (!subType.value) return ''
    const label = t(`subType.${subType.value}`)
    if (subInfo.value?.shape === 'pair' && pair.value) return `${pair.value.base}${pair.value.quote}`
    const current = binding.value
    const symbol = current && typeof current === 'object' && 'asset' in current ? current.asset.symbol : ''
    return symbol ? `${label} ${symbol}` : label
  })
  watch(suggestedName, (value) => {
    if (!nameTouched.value) name.value = value
  })

  const title = computed(() => {
    if (!props.parent) return step.value === 'pick' ? t('storeWizard.pickKind') : t('wallet.newStore')
    const path = props.parentPath?.length ? props.parentPath : [props.parent.name]
    return t('stores.newSubAccount', { parent: path.join(' › ') })
  })

  function pickKind(value: StoreKind) {
    kind.value = value
    color.value = KIND_COLORS[value] ?? DEFAULT_STORE_COLOR
    if (value === 'liability') terms.value = { ...emptyLiabilityTerms(), kind: 'loan' }
    step.value = 'details'
  }

  function pickSubType(value: SubType) {
    subType.value = value
    if (value === 'credit_card') terms.value = emptyLiabilityTerms()
    step.value = 'details'
  }

  watch(open, (isOpen) => {
    if (!isOpen) return
    const base = settings.baseCurrency.toLowerCase()
    name.value = ''
    nameTouched.value = false
    terms.value = emptyLiabilityTerms()
    custody.value = 'hot'
    provider.value = null
    apiKey.value = ''
    apiSecret.value = ''
    propertyCategory.value = 'flat'
    securedBy.value = ''
    currency.value = base
    token.value = null
    pairText.value = ''
    subType.value = null
    openingAmount.value = ''
    openingCurrency.value = base
    cardCurrency.value = base
    cardName.value = ''
    openingToken.value = null
    openingSide.value = 'quote'
    if (props.parent) {
      kind.value = props.parent.kind
      color.value = props.parent.color
      step.value = 'pick'
    } else if (props.kind) {
      pickKind(props.kind)
    } else {
      step.value = 'pick'
    }
  })

  const canGoBack = computed(() => step.value === 'details' && (isSub.value || !props.kind))

  const exchangeRoot = computed(() => !isSub.value && kind.value === 'exchange')
  /** Key and secret go together: one without the other cannot connect. */
  const keyHalf = computed(() => exchangeRoot.value && !apiKey.value.trim() !== !apiSecret.value.trim())
  const form = useFormValidation(
    () => ({
      name: required(name.value),
      token:
        isSub.value && subInfo.value?.shape === 'asset' && props.parent?.kind === 'crypto'
          ? required(token.value, 'pick')
          : null,
      currency:
        isSub.value && subInfo.value?.shape === 'asset' && props.parent?.kind !== 'crypto'
          ? required(currency.value, 'pick')
          : null,
      pair:
        isSub.value && subInfo.value?.shape === 'pair'
          ? (required(pairText.value) ?? (pair.value ? null : t('binding.pairInvalid')))
          : null,
      provider: exchangeRoot.value ? required(provider.value, 'pick') : null,
      apiKey: keyHalf.value && !apiKey.value.trim() ? t('validation.keyPair') : null,
      apiSecret: keyHalf.value && !apiSecret.value.trim() ? t('validation.keyPair') : null,
      cardCurrency: withCard.value ? required(cardCurrency.value, 'pick') : null,
      opening: openingValid.value ? null : t('storeWizard.openingInvalid'),
    }),
    { resetOn: [open, step] },
  )

  return {
    props,
    open,
    catalog,
    isSub,
    step,
    kind,
    subType,
    creatableKinds,
    subTypes,
    subInfo,
    name,
    nameTouched,
    color,
    terms,
    custody,
    provider,
    apiKey,
    apiSecret,
    withKey,
    propertyCategory,
    securedBy,
    currency,
    token,
    cardCurrency,
    cardName,
    withCard,
    pairText,
    pair,
    binding,
    openingAmount,
    openingCurrency,
    openingToken,
    openingSide,
    offersOpening,
    openingAsset,
    openingPicker,
    openingAmountValue,
    propertyRoots,
    title,
    pickKind,
    pickSubType,
    canGoBack,
    form,
  }
}

export type StoreCreateState = ReturnType<typeof setup>

const STORE_CREATE: InjectionKey<StoreCreateState> = Symbol('store-create')

/** Build the dialog's state and hand it to its sections. */
export function provideStoreCreate(props: StoreCreateDialogProps, open: Ref<boolean>): StoreCreateState {
  const state = setup(props, open)
  provide(STORE_CREATE, state)
  return state
}

/** The dialog a section belongs to; only inside `StoreCreateDialog`. */
export function useStoreCreate(): StoreCreateState {
  const state = inject(STORE_CREATE)
  if (!state) throw new Error('useStoreCreate() outside StoreCreateDialog')
  return state
}
