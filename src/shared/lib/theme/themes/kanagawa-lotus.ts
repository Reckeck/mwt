// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Kanagawa Lotus: mode light, radius 0.25rem.
 * Upstream palette: Kanagawa (https://github.com/rebelot/kanagawa.nvim), MIT licence, source https://github.com/rebelot/kanagawa.nvim.
 * tuned from upstream: --muted-foreground, #8A8980
 * tuned from upstream: --success, #6F894E
 * tuned from upstream: --warning, #CC6D00
 * tuned from upstream: --money-income, #6F894E
 * tuned from upstream: --money-expense, #C84053
 * tuned from upstream: --money-transfer, #4D699B
 * tuned from upstream: --money-in-transit, #CC6D00
 * tuned from upstream: --destructive, #C84053
 */
export const kanagawaLotus: Theme = {
  id: 'kanagawa-lotus',
  name: 'Kanagawa Lotus',
  mode: 'light',
  tokens: {
    '--background': '#F2ECBC',
    '--foreground': '#545464',
    '--card': '#F6F1CB',
    '--card-foreground': '#545464',
    '--popover': '#F6F1CB',
    '--popover-foreground': '#545464',
    '--primary': '#4D699B',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#E5DDB0',
    '--secondary-foreground': '#545464',
    '--muted': '#E5DDB0',
    '--muted-foreground': '#605E52',
    '--accent': '#E5DDB0',
    '--accent-foreground': '#545464',
    '--destructive': '#AA3747',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#516438',
    '--success-foreground': '#FFFFFF',
    '--warning': '#934F00',
    '--warning-foreground': '#FFFFFF',
    '--border': '#D5CEA3',
    '--input': '#D5CEA3',
    '--ring': '#4D699B',
    '--surface-inverse': '#545464',
    '--inverse-foreground': '#F2ECBC',
    '--success-soft': '#D5D4A4',
    '--warning-soft': '#E1D09A',
    '--destructive-soft': '#E5CBA7',
    '--scrim': '#0000005C',
    '--text-tertiary': '#8A8980',
    '--border-strong': '#AEA990',
    '--sidebar': '#F6F1CB',
    '--sidebar-foreground': '#545464',
    '--sidebar-primary': '#4D699B',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#E5DDB0',
    '--sidebar-accent-foreground': '#545464',
    '--sidebar-border': '#D5CEA3',
    '--sidebar-ring': '#4D699B',
    '--chart-1': '#4D699B',
    '--chart-2': '#6F894E',
    '--chart-3': '#CC6D00',
    '--chart-4': '#624C83',
    '--chart-5': '#C84053',
    '--chart-actual': '#545464',
    '--chart-reference': '#8A8980',
    '--chart-bank': '#6F894E',
    '--chart-crypto': '#CC6D00',
    '--chart-property': '#597B75',
    '--chart-accent': '#B35B79',
    '--chart-income': '#6F894E',
    '--chart-over': '#C84053',
    '--chart-grid': '#E2DCB3',
    '--money-income': '#516438',
    '--money-expense': '#AA3747',
    '--money-transfer': '#47618F',
    '--money-in-transit': '#934F00',
    '--chart-band': '#8A898059',
    '--chart-wash': '#5454641A',
    '--chart-heat-0': '#E2E1C5',
    '--chart-heat-1': '#BBC1BA',
    '--chart-heat-2': '#919FAE',
    '--chart-heat-3': '#6F84A5',
    '--chart-heat-4': '#4D699B',
    '--radius': '0.25rem',
  },
}
