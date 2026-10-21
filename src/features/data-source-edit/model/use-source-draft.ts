import { ref } from 'vue'

import { decimal, required } from '@/shared/lib/form'
import { i18n } from '@/shared/config/i18n'
import { assetKey } from '@/shared/lib/asset-ref'

export type MethodKind = SourceMethod['kind']
export type SourceField = 'name' | 'url' | 'target' | 'column' | 'multiplier'

/** What a use of the source needs from the form: one currency, one asset, one deposit. */
export interface UseDraft {
  inflation: string
  asset: AssetRef | null
  assetKey: string
  priceCurrency: string
  depositStoreId: string
}

/**
 * The form of a data source: address, how to find the value, how to read
 * the number, how often, and where the value goes.
 */
export function useSourceDraft() {
  const id = ref<string | null>(null)
  const name = ref('')
  const url = ref('')
  const kind = ref<MethodKind>('css')
  const selector = ref('')
  const textHint = ref('')
  const column = ref('')
  const row = ref<TableRow>('by_date')
  const path = ref('')
  const label = ref('')
  const fileFormat = ref<FileFormat>('csv')
  const sheet = ref('')
  const output = ref<SourceOutput>('number')
  const decimalComma = ref(true)
  const multiplier = ref('1')
  const strip = ref('')
  const interval = ref<SourceInterval>('day')
  const uses = ref<UseDraft>({ inflation: '', asset: null, assetKey: '', priceCurrency: 'usd', depositStoreId: '' })
  const workflowUses = ref<SourceUse[]>([])

  function load(view: DataSourceView | null) {
    const source = view?.source
    id.value = source?.id ?? null
    name.value = source?.name ?? ''
    url.value = source?.url ?? ''
    output.value = source?.output ?? 'number'
    decimalComma.value = source?.number.decimal_comma ?? true
    multiplier.value = source?.number.multiplier ?? '1'
    strip.value = source?.number.strip ?? ''
    interval.value = source?.interval ?? 'day'
    selector.value = ''
    textHint.value = ''
    column.value = ''
    row.value = 'by_date'
    path.value = ''
    label.value = ''
    sheet.value = ''
    kind.value = source?.method.kind ?? 'css'
    const method = source?.method
    if (method?.kind === 'css') {
      selector.value = method.selector
      textHint.value = method.text_hint
    } else if (method?.kind === 'table') {
      selector.value = method.table_selector
      column.value = method.column_header
      row.value = method.row
    } else if (method?.kind === 'json') path.value = method.path
    else if (method?.kind === 'label') label.value = method.label
    else if (method?.kind === 'file') {
      fileFormat.value = method.format
      sheet.value = method.sheet
      column.value = method.column
      row.value = method.row
    }
    const used = view?.used_by ?? []
    const inflation = used.find((usage) => usage.kind === 'inflation')
    const price = used.find((usage) => usage.kind === 'asset_price')
    const deposit = used.find((usage) => usage.kind === 'deposit_rate')
    uses.value = {
      inflation: inflation?.kind === 'inflation' ? inflation.currency : '',
      asset: null,
      assetKey: price?.kind === 'asset_price' ? price.asset_key : '',
      priceCurrency: price?.kind === 'asset_price' ? price.currency : 'usd',
      depositStoreId: deposit?.kind === 'deposit_rate' ? deposit.store_id : '',
    }
    workflowUses.value = used.filter((usage) => usage.kind === 'workflow')
  }

  function method(): SourceMethod {
    switch (kind.value) {
      case 'table':
        return {
          kind: 'table',
          table_selector: selector.value.trim(),
          column_header: column.value.trim(),
          row: row.value,
        }
      case 'json':
        return { kind: 'json', path: path.value.trim() }
      case 'label':
        return { kind: 'label', label: label.value.trim() }
      case 'file':
        return {
          kind: 'file',
          format: fileFormat.value,
          sheet: sheet.value.trim(),
          column: column.value.trim(),
          row: row.value,
        }
      default:
        return { kind: 'css', selector: selector.value.trim(), text_hint: textHint.value.trim() }
    }
  }

  function toUses(): SourceUse[] {
    const out: SourceUse[] = []
    const draft = uses.value
    if (draft.inflation) out.push({ kind: 'inflation', currency: draft.inflation })
    const key = draft.asset ? assetKey(draft.asset) : draft.assetKey
    if (key) out.push({ kind: 'asset_price', asset_key: key, currency: draft.priceCurrency || 'usd' })
    if (draft.depositStoreId) out.push({ kind: 'deposit_rate', store_id: draft.depositStoreId })
    return out
  }

  function toDraft(): DataSourceDraft {
    return {
      id: id.value,
      name: name.value.trim(),
      url: url.value.trim(),
      method: method(),
      output: output.value,
      number: { decimal_comma: decimalComma.value, multiplier: multiplier.value || '1', strip: strip.value },
      interval: interval.value,
      uses: toUses(),
    }
  }

  /** The target of the method: a selector, a path, a label or a column. */
  function targetMissing(): string | null {
    const methodNow = method()
    const value =
      methodNow.kind === 'css'
        ? methodNow.selector
        : methodNow.kind === 'table'
          ? methodNow.table_selector
          : methodNow.kind === 'json'
            ? methodNow.path
            : methodNow.kind === 'label'
              ? methodNow.label
              : 'file'
    return required(value)
  }

  function rules(): Record<SourceField, string | null> {
    const columnNeeded = kind.value === 'table' || kind.value === 'file'
    return {
      name: required(name.value),
      url: required(url.value) ?? (/^https?:\/\//i.test(url.value.trim()) ? null : i18n.global.t('validation.url')),
      target: targetMissing(),
      column: columnNeeded ? required(column.value) : null,
      multiplier: required(multiplier.value) ?? decimal(multiplier.value),
    }
  }

  return {
    id,
    name,
    url,
    kind,
    selector,
    textHint,
    column,
    row,
    path,
    label,
    fileFormat,
    sheet,
    output,
    decimalComma,
    multiplier,
    strip,
    interval,
    uses,
    workflowUses,
    load,
    toDraft,
    rules,
  }
}

export type SourceDraftState = ReturnType<typeof useSourceDraft>
