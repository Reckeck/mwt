// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Nord Snow Storm: mode light, radius 0.375rem.
 * Upstream palette: Nord, Snow Storm light arrangement (https://www.nordtheme.com/docs/colors-and-palettes), MIT licence, source https://github.com/nordtheme/nord.
 * tuned from upstream: --money-income, #A3BE8C
 * tuned from upstream: --money-expense, #BF616A
 * tuned from upstream: --money-transfer, #5E81AC
 * tuned from upstream: --money-in-transit, #EBCB8B
 * tuned from upstream: --success, #A3BE8C
 * tuned from upstream: --destructive, #BF616A
 * tuned from upstream: --warning, #EBCB8B
 * tuned from upstream: --primary-foreground, #FFFFFF
 * tuned from upstream: --sidebar-primary-foreground, #FFFFFF
 * tuned from upstream: --chart-2, #A3BE8C
 * tuned from upstream: --chart-3, #D08770
 * tuned from upstream: --chart-4, #B48EAD
 * tuned from upstream: --chart-bank, #A3BE8C
 * tuned from upstream: --chart-crypto, #D08770
 * tuned from upstream: --chart-accent, #B48EAD
 * tuned from upstream: --chart-income, #A3BE8C
 */
export const nordSnowStorm: Theme = {
  id: 'nord-snow-storm',
  name: 'Nord Snow Storm',
  mode: 'light',
  tokens: {
    '--background': '#ECEFF4',
    '--foreground': '#2E3440',
    '--card': '#FFFFFF',
    '--card-foreground': '#2E3440',
    '--popover': '#FFFFFF',
    '--popover-foreground': '#2E3440',
    '--primary': '#5E81AC',
    '--primary-foreground': '#151515',
    '--secondary': '#E5E9F0',
    '--secondary-foreground': '#2E3440',
    '--muted': '#E5E9F0',
    '--muted-foreground': '#4C566A',
    '--accent': '#E5E9F0',
    '--accent-foreground': '#2E3440',
    '--destructive': '#9F5058',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#5C6B4F',
    '--success-foreground': '#FFFFFF',
    '--warning': '#746546',
    '--warning-foreground': '#FFFFFF',
    '--border': '#D1D5DB',
    '--input': '#D1D5DB',
    '--ring': '#5E81AC',
    '--surface-inverse': '#2E3440',
    '--inverse-foreground': '#ECEFF4',
    '--success-soft': '#E0E7E3',
    '--warning-soft': '#ECE9E3',
    '--destructive-soft': '#E5D8DE',
    '--scrim': '#0000005C',
    '--text-tertiary': '#848C9A',
    '--border-strong': '#B3B7BE',
    '--sidebar': '#FFFFFF',
    '--sidebar-foreground': '#2E3440',
    '--sidebar-primary': '#5E81AC',
    '--sidebar-primary-foreground': '#151515',
    '--sidebar-accent': '#E5E9F0',
    '--sidebar-accent-foreground': '#2E3440',
    '--sidebar-border': '#D1D5DB',
    '--sidebar-ring': '#5E81AC',
    '--chart-1': '#5E81AC',
    '--chart-2': '#849971',
    '--chart-3': '#CA836D',
    '--chart-4': '#AA86A3',
    '--chart-5': '#BF616A',
    '--chart-actual': '#2E3440',
    '--chart-reference': '#A0A4AC',
    '--chart-bank': '#849971',
    '--chart-crypto': '#CA836D',
    '--chart-property': '#5E81AC',
    '--chart-accent': '#AA86A3',
    '--chart-income': '#849971',
    '--chart-over': '#BF616A',
    '--chart-grid': '#D5D9DE',
    '--money-income': '#5C6B4F',
    '--money-expense': '#9F5058',
    '--money-transfer': '#4E6B8F',
    '--money-in-transit': '#746546',
    '--chart-band': '#A0A4AC59',
    '--chart-wash': '#2E34401A',
    '--chart-heat-0': '#E1E6EE',
    '--chart-heat-1': '#C9D4E2',
    '--chart-heat-2': '#A5B8D0',
    '--chart-heat-3': '#829DBE',
    '--chart-heat-4': '#5E81AC',
    '--radius': '0.375rem',
  },
}
