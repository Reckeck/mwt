// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Nord: mode dark, radius 0.375rem.
 * Upstream palette: Nord (https://www.nordtheme.com/docs/colors-and-palettes), MIT licence, source https://github.com/nordtheme/nord.
 * tuned from upstream: --money-income, #A3BE8C
 * tuned from upstream: --money-expense, #BF616A
 * tuned from upstream: --money-transfer, #81A1C1
 * tuned from upstream: --success, #A3BE8C
 * tuned from upstream: --destructive, #BF616A
 * tuned from upstream: --chart-5, #BF616A
 * tuned from upstream: --chart-over, #BF616A
 */
export const nord: Theme = {
  id: 'nord',
  name: 'Nord',
  mode: 'dark',
  tokens: {
    '--background': '#2E3440',
    '--foreground': '#ECEFF4',
    '--card': '#3B4252',
    '--card-foreground': '#ECEFF4',
    '--popover': '#3B4252',
    '--popover-foreground': '#ECEFF4',
    '--primary': '#88C0D0',
    '--primary-foreground': '#2E3440',
    '--secondary': '#434C5E',
    '--secondary-foreground': '#ECEFF4',
    '--muted': '#434C5E',
    '--muted-foreground': '#D8DEE9',
    '--accent': '#434C5E',
    '--accent-foreground': '#ECEFF4',
    '--destructive': '#DEB0B4',
    '--destructive-foreground': '#2E3440',
    '--success': '#ACC495',
    '--success-foreground': '#2E3440',
    '--warning': '#EBCB8B',
    '--warning-foreground': '#2E3440',
    '--border': '#494E59',
    '--input': '#494E59',
    '--ring': '#88C0D0',
    '--surface-inverse': '#ECEFF4',
    '--inverse-foreground': '#2E3440',
    '--success-soft': '#45504F',
    '--warning-soft': '#54524F',
    '--destructive-soft': '#4B3D48',
    '--scrim': '#000000A6',
    '--text-tertiary': '#9DA3AE',
    '--border-strong': '#676C76',
    '--sidebar': '#3B4252',
    '--sidebar-foreground': '#ECEFF4',
    '--sidebar-primary': '#88C0D0',
    '--sidebar-primary-foreground': '#2E3440',
    '--sidebar-accent': '#434C5E',
    '--sidebar-accent-foreground': '#ECEFF4',
    '--sidebar-border': '#494E59',
    '--sidebar-ring': '#88C0D0',
    '--chart-1': '#81A1C1',
    '--chart-2': '#A3BE8C',
    '--chart-3': '#D08770',
    '--chart-4': '#B48EAD',
    '--chart-5': '#C9777E',
    '--chart-actual': '#ECEFF4',
    '--chart-reference': '#7A7F88',
    '--chart-bank': '#A3BE8C',
    '--chart-crypto': '#D08770',
    '--chart-property': '#81A1C1',
    '--chart-accent': '#B48EAD',
    '--chart-income': '#A3BE8C',
    '--chart-over': '#C9777E',
    '--chart-grid': '#454A56',
    '--money-income': '#ACC495',
    '--money-expense': '#DEB0B4',
    '--money-transfer': '#ABBFD5',
    '--money-in-transit': '#EBCB8B',
    '--chart-band': '#7A7F8859',
    '--chart-wash': '#ECEFF41A',
    '--chart-heat-0': '#353F4C',
    '--chart-heat-1': '#455764',
    '--chart-heat-2': '#5B7A88',
    '--chart-heat-3': '#729DAC',
    '--chart-heat-4': '#88C0D0',
    '--radius': '0.375rem',
  },
}
