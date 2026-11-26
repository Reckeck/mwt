// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Rosé Pine Dawn: mode light, radius 0.75rem.
 * Upstream palette: Rosé Pine Dawn (https://rosepinetheme.com/palette), MIT licence, source https://github.com/rose-pine/rose-pine-theme.
 * tuned from upstream: --muted-foreground, #797593
 * tuned from upstream: --destructive, #B4637A
 * tuned from upstream: --destructive-foreground, #151515
 * tuned from upstream: --warning, #EA9D34
 * tuned from upstream: --chart-3, #D7827E
 * tuned from upstream: --chart-crypto, #D7827E
 * tuned from upstream: --money-expense, #B4637A
 * tuned from upstream: --money-in-transit, #EA9D34
 */
export const rosePineDawn: Theme = {
  id: 'rose-pine-dawn',
  name: 'Rosé Pine Dawn',
  mode: 'light',
  tokens: {
    '--background': '#FAF4ED',
    '--foreground': '#575279',
    '--card': '#FFFAF3',
    '--card-foreground': '#575279',
    '--popover': '#FFFAF3',
    '--popover-foreground': '#575279',
    '--primary': '#907AA9',
    '--primary-foreground': '#151515',
    '--secondary': '#F2E9E1',
    '--secondary-foreground': '#575279',
    '--muted': '#F2E9E1',
    '--muted-foreground': '#6E6B87',
    '--accent': '#F2E9E1',
    '--accent-foreground': '#575279',
    '--destructive': '#9B5469',
    '--destructive-foreground': '#EAEAEA',
    '--success': '#286983',
    '--success-foreground': '#FFFFFF',
    '--warning': '#8C5E22',
    '--warning-foreground': '#151515',
    '--border': '#DFDAD9',
    '--input': '#DFDAD9',
    '--ring': '#907AA9',
    '--surface-inverse': '#575279',
    '--inverse-foreground': '#FAF4ED',
    '--success-soft': '#E1E6E3',
    '--warning-soft': '#FCEDD8',
    '--destructive-soft': '#F5E5E2',
    '--scrim': '#0000005C',
    '--text-tertiary': '#9893A7',
    '--border-strong': '#B6B1BC',
    '--sidebar': '#FFFAF3',
    '--sidebar-foreground': '#575279',
    '--sidebar-primary': '#907AA9',
    '--sidebar-primary-foreground': '#151515',
    '--sidebar-accent': '#F2E9E1',
    '--sidebar-accent-foreground': '#575279',
    '--sidebar-border': '#DFDAD9',
    '--sidebar-ring': '#907AA9',
    '--chart-1': '#286983',
    '--chart-2': '#286983',
    '--chart-3': '#CB7A76',
    '--chart-4': '#907AA9',
    '--chart-5': '#B4637A',
    '--chart-actual': '#575279',
    '--chart-reference': '#A9A3B3',
    '--chart-bank': '#286983',
    '--chart-crypto': '#CB7A76',
    '--chart-property': '#286983',
    '--chart-accent': '#907AA9',
    '--chart-income': '#286983',
    '--chart-over': '#B4637A',
    '--chart-grid': '#EBE6E4',
    '--chart-band': '#A9A3B359',
    '--chart-wash': '#5752791A',
    '--chart-heat-0': '#F2EBEA',
    '--chart-heat-1': '#DED4DD',
    '--chart-heat-2': '#C8BACE',
    '--chart-heat-3': '#AC9ABC',
    '--chart-heat-4': '#907AA9',
    '--money-income': '#286983',
    '--money-expense': '#9B5469',
    '--money-transfer': '#286983',
    '--money-in-transit': '#8C5E22',
    '--radius': '0.75rem',
  },
}
