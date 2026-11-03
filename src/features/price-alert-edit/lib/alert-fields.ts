/** The alert form's fields as plain values (what the refs of `PriceAlertDialog` hold). */
export interface AlertFields {
  asset: AssetRef | null
  condition: AlertCondition
  price: string
  currency: string
  once: boolean
}

/** The fields a stored alert fills in the form. */
export function fieldsOfAlert(alert: PriceAlert): AlertFields {
  return {
    asset: alert.asset,
    condition: alert.condition,
    price: alert.price,
    currency: alert.currency,
    once: !alert.repeats,
  }
}

const identOf = (asset: AssetRef): AssetIdent => ({
  symbol: asset.symbol,
  name: asset.name,
  coingecko_id: asset.coingecko_id,
  chain: asset.chain,
  contract: asset.contract,
})

/** The input of an alert as the core stores it: the edit form's base (an `PriceAlert` carries more on top). */
export function alertInputOf(alert: PriceAlert): PriceAlertInput {
  return {
    asset: identOf(alert.asset),
    condition: alert.condition,
    price: alert.price,
    currency: alert.currency,
    active: alert.active,
    store_id: alert.store_id,
    repeats: alert.repeats,
  }
}

/** `active` and `store_id` are not on the form: they come from the alert being edited or the opener. */
export function alertInputOfFields(
  fields: AlertFields,
  rest: { active: boolean; storeId: string | undefined },
): PriceAlertInput {
  const picked = fields.asset as AssetRef
  return {
    asset: identOf(picked),
    condition: fields.condition,
    price: fields.price.trim(),
    currency: fields.currency,
    active: rest.active,
    store_id: rest.storeId,
    repeats: !fields.once,
  }
}
