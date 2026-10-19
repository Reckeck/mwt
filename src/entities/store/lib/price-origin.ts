import { i18n } from '@/shared/config/i18n'
import { formatDateTime } from '@/shared/lib/format'

/** Provider ids as people know them. */
const PROVIDER_NAMES: Record<string, string> = {
  coingecko: 'CoinGecko',
  'coingecko-token': 'CoinGecko',
  coincap: 'CoinCap',
  binance: 'Binance',
  'open.er-api': 'ExchangeRate-API',
}

/** «CoinGecko · 19 окт., 12:05», «своя цена», «по привязке 1 USD»… — said next to a price. */
export function priceOriginText(info: PriceInfo): string {
  const t = i18n.global.t
  const at = info.at ? formatDateTime(info.at) : ''
  switch (info.origin) {
    case 'market': {
      const name = info.source ? (PROVIDER_NAMES[info.source] ?? info.source) : t('price.source.unknown')
      return at ? `${name} · ${at}` : name
    }
    case 'fiat':
      return at ? `${t('price.origin.fiat')} · ${at}` : t('price.origin.fiat')
    case 'stamp':
      return at ? `${t('price.origin.stamp')} · ${at}` : t('price.origin.stamp')
    default:
      return t(`price.origin.${info.origin}`)
  }
}
