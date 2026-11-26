// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Gruvbox dark: mode dark, radius 0.25rem.
 * Upstream palette: Gruvbox (https://github.com/morhetz/gruvbox), MIT licence, source https://github.com/morhetz/gruvbox.
 * tuned from upstream: --primary-foreground, #1D2021
 * tuned from upstream: --destructive, #FB4934
 * tuned from upstream: --destructive-foreground, #1D2021
 * tuned from upstream: --success-foreground, #1D2021
 * tuned from upstream: --warning-foreground, #1D2021
 * tuned from upstream: --money-expense, #FB4934
 * tuned from upstream: --money-transfer, #83A598
 */
export const gruvboxDark: Theme = {
  id: 'gruvbox-dark',
  name: 'Gruvbox dark',
  mode: 'dark',
  tokens: {
    '--background': '#282828',
    '--foreground': '#EBDBB2',
    '--card': '#32302F',
    '--card-foreground': '#EBDBB2',
    '--popover': '#32302F',
    '--popover-foreground': '#EBDBB2',
    '--primary': '#83A598',
    '--primary-foreground': '#000000',
    '--secondary': '#3C3836',
    '--secondary-foreground': '#EBDBB2',
    '--muted': '#3C3836',
    '--muted-foreground': '#A89984',
    '--accent': '#3C3836',
    '--accent-foreground': '#EBDBB2',
    '--destructive': '#FC7C6D',
    '--destructive-foreground': '#000000',
    '--success': '#B8BB26',
    '--success-foreground': '#000000',
    '--warning': '#FABD2F',
    '--warning-foreground': '#000000',
    '--border': '#504945',
    '--input': '#504945',
    '--ring': '#83A598',
    '--surface-inverse': '#EBDBB2',
    '--inverse-foreground': '#282828',
    '--success-soft': '#3E3E28',
    '--warning-soft': '#483E29',
    '--destructive-soft': '#482D2A',
    '--scrim': '#000000A6',
    '--text-tertiary': '#938A74',
    '--border-strong': '#867C6B',
    '--sidebar': '#32302F',
    '--sidebar-foreground': '#EBDBB2',
    '--sidebar-primary': '#83A598',
    '--sidebar-primary-foreground': '#000000',
    '--sidebar-accent': '#3C3836',
    '--sidebar-accent-foreground': '#EBDBB2',
    '--sidebar-border': '#504945',
    '--sidebar-ring': '#83A598',
    '--chart-1': '#83A598',
    '--chart-2': '#B8BB26',
    '--chart-3': '#FE8019',
    '--chart-4': '#D3869B',
    '--chart-5': '#FB4934',
    '--chart-actual': '#EBDBB2',
    '--chart-reference': '#807966',
    '--chart-bank': '#B8BB26',
    '--chart-crypto': '#FE8019',
    '--chart-property': '#83A598',
    '--chart-accent': '#D3869B',
    '--chart-income': '#B8BB26',
    '--chart-over': '#FB4934',
    '--chart-grid': '#443F3C',
    '--chart-band': '#80796659',
    '--chart-wash': '#EBDBB21A',
    '--chart-heat-0': '#333735',
    '--chart-heat-1': '#47534E',
    '--chart-heat-2': '#5B6E67',
    '--chart-heat-3': '#6F8A7F',
    '--chart-heat-4': '#83A598',
    '--money-income': '#B8BB26',
    '--money-expense': '#FC7C6D',
    '--money-transfer': '#88A99C',
    '--money-in-transit': '#FABD2F',
    '--radius': '0.25rem',
  },
}
