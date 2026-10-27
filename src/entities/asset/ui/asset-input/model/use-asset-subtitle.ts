import { useI18n } from 'vue-i18n'

import { CHAIN_LABELS } from '@/shared/lib/chains'

/** The line under the chosen asset: a fiat currency's name, or a coin's name and chain. */
export function useAssetSubtitle(filterFiat: (query: string) => { code: string; name: string }[]) {
  const { t } = useI18n()
  return (value: AssetRef) => {
    if (value.chain === 'fiat') {
      const name = filterFiat(value.symbol).find((row) => row.code.toUpperCase() === value.symbol)?.name
      return name ? `${name} · ${t('currency.kind.fiat')}` : t('currency.kind.fiat')
    }
    return value.chain ? `${value.name} · ${CHAIN_LABELS[value.chain]}` : value.name
  }
}
