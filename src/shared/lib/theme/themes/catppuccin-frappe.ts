// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Catppuccin Frappé: mode dark, radius 0.75rem.
 * Upstream palette: Catppuccin Frappé (https://catppuccin.com/palette), MIT licence, source https://github.com/catppuccin/catppuccin.
 * tuned from upstream: --primary-foreground, #232634
 * tuned from upstream: --destructive, #E78284
 * tuned from upstream: --destructive-foreground, #232634
 * tuned from upstream: --success-foreground, #232634
 * tuned from upstream: --warning-foreground, #232634
 * tuned from upstream: --money-expense, #E78284
 * tuned from upstream: --money-transfer, #8CAAEE
 */
export const catppuccinFrappe: Theme = {
  id: 'catppuccin-frappe',
  name: 'Catppuccin Frappé',
  mode: 'dark',
  tokens: {
    '--background': '#303446',
    '--foreground': '#C6D0F5',
    '--card': '#292C3C',
    '--card-foreground': '#C6D0F5',
    '--popover': '#292C3C',
    '--popover-foreground': '#C6D0F5',
    '--primary': '#CA9EE6',
    '--primary-foreground': '#000000',
    '--secondary': '#414559',
    '--secondary-foreground': '#C6D0F5',
    '--muted': '#414559',
    '--muted-foreground': '#A5ADCE',
    '--accent': '#414559',
    '--accent-foreground': '#C6D0F5',
    '--destructive': '#ED9FA0',
    '--destructive-foreground': '#000000',
    '--success': '#A6D189',
    '--success-foreground': '#000000',
    '--warning': '#E5C890',
    '--warning-foreground': '#000000',
    '--border': '#51576D',
    '--input': '#51576D',
    '--ring': '#CA9EE6',
    '--surface-inverse': '#C6D0F5',
    '--inverse-foreground': '#303446',
    '--success-soft': '#424C50',
    '--warning-soft': '#4B4A51',
    '--destructive-soft': '#4B404F',
    '--scrim': '#000000A6',
    '--text-tertiary': '#838AA6',
    '--border-strong': '#7A819D',
    '--sidebar': '#292C3C',
    '--sidebar-foreground': '#C6D0F5',
    '--sidebar-primary': '#CA9EE6',
    '--sidebar-primary-foreground': '#000000',
    '--sidebar-accent': '#414559',
    '--sidebar-accent-foreground': '#C6D0F5',
    '--sidebar-border': '#51576D',
    '--sidebar-ring': '#CA9EE6',
    '--chart-1': '#8CAAEE',
    '--chart-2': '#A6D189',
    '--chart-3': '#EF9F76',
    '--chart-4': '#CA9EE6',
    '--chart-5': '#E78284',
    '--chart-actual': '#C6D0F5',
    '--chart-reference': '#747A95',
    '--chart-bank': '#A6D189',
    '--chart-crypto': '#EF9F76',
    '--chart-property': '#8CAAEE',
    '--chart-accent': '#CA9EE6',
    '--chart-income': '#A6D189',
    '--chart-over': '#E78284',
    '--chart-grid': '#474D61',
    '--chart-band': '#747A9559',
    '--chart-wash': '#C6D0F51A',
    '--chart-heat-0': '#424159',
    '--chart-heat-1': '#64587C',
    '--chart-heat-2': '#866FA0',
    '--chart-heat-3': '#A887C3',
    '--chart-heat-4': '#CA9EE6',
    '--money-income': '#A6D189',
    '--money-expense': '#ED9FA0',
    '--money-transfer': '#98B3F0',
    '--money-in-transit': '#E5C890',
    '--radius': '0.75rem',
  },
}
