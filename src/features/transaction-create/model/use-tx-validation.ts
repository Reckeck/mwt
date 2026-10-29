import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { parseDecimal } from '@/shared/lib/decimal'
import { formatQuantity } from '@/shared/lib/format'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'

import { isFormComplete, missingFields } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

/** Fields entered as numbers: blank, not a number and «≤ 0» each get their own message. */
const AMOUNT_FIELDS = new Set([
  'amount',
  'fromAmount',
  'toAmount',
  'soldQty',
  'boughtQty',
  'quantity',
  'fillPrice',
  'sellQty',
  'sellPrice',
])

/** Fields that hold a wallet or a person: `Pick a wallet`, not `Pick one`. */
const WALLET_FIELDS = new Set(['from', 'to', 'via'])
const ASSET_FIELDS = new Set(['asset', 'fromAsset', 'toAsset', 'soldAsset', 'boughtAsset', 'currency'])

/**
 * What the form still lacks: a message under each missing field after a save
 * was tried, the summary line under the form, and the category a top-up or a
 * payment needs unless a rule fills it in (backend check V5).
 */
export function useTxValidation(state: TxFormState) {
  const { t } = useI18n()
  const { props, form, contacts, serverErrors, overview, editing } = state

  /** What is still missing, said under the form. */
  const missing = computed(() => (props.template || props.disabled ? [] : missingFields(form)))
  const valid = computed(() => !props.disabled && isFormComplete(form))

  /** An exchange of a currency for itself is a transfer between wallets (core: «a conversion needs two different assets»). */
  const sameCurrency = computed(
    () =>
      form.kind === 'convert' &&
      Boolean(form.fromAsset && form.toAsset) &&
      assetKey(form.fromAsset as AssetRef) === assetKey(form.toAsset as AssetRef),
  )

  /** Staking or a pool pays out of this wallet: more than it holds is refused here, not by the backend. */
  const overAvailable = computed(() => {
    if (form.kind !== 'defi' || editing.value || !form.asset || !overview.data.value) return ''
    const amount = Number(parseDecimal(form.amount) ?? Number.NaN)
    if (!(amount > 0)) return ''
    const key = assetKey(form.asset)
    const held = overview.data.value.positions.find((position) => assetKey(position.asset) === key)?.quantity ?? '0'
    if (amount <= Number(held)) return ''
    return t('validation.overAvailable', { amount: `${formatQuantity(held)} ${form.asset.symbol.toUpperCase()}` })
  })

  const needsCategory = computed(() => form.kind === 'topup' || form.kind === 'payment')
  const rules = useQuery({
    queryKey: ['categorization-rules'],
    queryFn: () => call<CategorizationRule[]>('list_categorization_rules'),
    enabled: needsCategory,
  })
  /** Same match as `apply_categorization_rules`: a pattern inside the comment or a contact's name. */
  const ruleFillsCategory = computed(() => {
    const contactIds = [
      form.contactId,
      form.counterpart && 'contact' in form.counterpart ? form.counterpart.contact.contact_id : '',
    ]
    const names = (contacts.data.value ?? [])
      .filter((contact) => contactIds.includes(contact.id))
      .map((contact) => contact.name)
    const haystacks = [form.comment, ...names].map((text) => text.toLowerCase())
    return (rules.data.value ?? []).some(
      (rule) =>
        rule.active &&
        rule.pattern.trim() &&
        haystacks.some((text) => text.includes(rule.pattern.trim().toLowerCase())),
    )
  })

  /** Every missing field gets its message under it; the list under the form stays as the summary. */
  const check = useFormValidation(
    () => {
      if (props.disabled) return {}
      const errors: Record<string, string | null> = {}
      // A template hides the summary list, but its fields are checked all the same.
      for (const key of props.template ? missingFields(form) : missing.value) {
        if (AMOUNT_FIELDS.has(key)) errors[key] = positiveAmount(form[key as keyof typeof form] as string)
        else if (key === 'symbol') errors[key] = required(form.symbol)
        else if (key === 'extra') errors[key] = t('validation.splitRows')
        else if (WALLET_FIELDS.has(key)) errors[key] = required(null, 'pickStore')
        else if (ASSET_FIELDS.has(key)) errors[key] = required(null, 'pickAsset')
        else errors[key] = required(null, 'pick')
      }
      if (needsCategory.value && !props.template && !form.categories.length && !ruleFillsCategory.value) {
        errors.categories = t('validation.category')
      }
      if (overAvailable.value && !errors.amount) errors.amount = overAvailable.value
      if (sameCurrency.value) errors.toAsset = t('validation.sameCurrency')
      for (const [field, text] of Object.entries(serverErrors.value)) errors[field] ??= text
      // Complete on the surface but still not an operation (e.g. an unknown kind of side).
      if (!Object.values(errors).some(Boolean) && !isFormComplete(form)) errors.form = t('validation.summary')
      return errors
    },
    { resetOn: () => form.kind },
  )

  return { missing, valid, needsCategory, sameCurrency, overAvailable, check }
}
