// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Catppuccin Latte: mode light, radius 0.75rem.
 * Upstream palette: Catppuccin Latte (https://catppuccin.com/palette), MIT licence, source https://github.com/catppuccin/catppuccin.
 * tuned from upstream: --primary-foreground, #EFF1F5
 * tuned from upstream: --muted-foreground, #6C6F85
 * tuned from upstream: --destructive, #D20F39
 * tuned from upstream: --destructive-foreground, #EFF1F5
 * tuned from upstream: --success, #40A02B
 * tuned from upstream: --success-foreground, #EFF1F5
 * tuned from upstream: --warning, #DF8E1D
 * tuned from upstream: --warning-foreground, #EFF1F5
 * tuned from upstream: --chart-3, #FE640B
 * tuned from upstream: --money-income, #40A02B
 * tuned from upstream: --money-expense, #D20F39
 * tuned from upstream: --money-transfer, #1E66F5
 * tuned from upstream: --money-in-transit, #DF8E1D
 */
export const catppuccinLatte: Theme = {
  id: 'catppuccin-latte',
  name: 'Catppuccin Latte',
  mode: 'light',
  tokens: {
    '--background': '#EFF1F5',
    '--foreground': '#4C4F69',
    '--card': '#F7F8FA',
    '--card-foreground': '#4C4F69',
    '--popover': '#F7F8FA',
    '--popover-foreground': '#4C4F69',
    '--primary': '#8839ef',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#CCD0DA',
    '--secondary-foreground': '#4C4F69',
    '--muted': '#CCD0DA',
    '--muted-foreground': '#6A6D82',
    '--accent': '#CCD0DA',
    '--accent-foreground': '#4C4F69',
    '--destructive': '#B30D30',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#29661C',
    '--success-foreground': '#FFFFFF',
    '--warning': '#7B4E10',
    '--warning-foreground': '#FFFFFF',
    '--border': '#BCC0CC',
    '--input': '#BCC0CC',
    '--ring': '#8839ef',
    '--surface-inverse': '#4C4F69',
    '--inverse-foreground': '#EFF1F5',
    '--success-soft': '#D5E5D7',
    '--warning-soft': '#EDE2D5',
    '--destructive-soft': '#EBCFD9',
    '--scrim': '#0000005C',
    '--text-tertiary': '#9598A8',
    '--border-strong': '#9598A9',
    '--sidebar': '#F7F8FA',
    '--sidebar-foreground': '#4C4F69',
    '--sidebar-primary': '#8839ef',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#CCD0DA',
    '--sidebar-accent-foreground': '#4C4F69',
    '--sidebar-border': '#BCC0CC',
    '--sidebar-ring': '#8839ef',
    '--chart-1': '#1E66F5',
    '--chart-2': '#40A02B',
    '--chart-3': '#F4600B',
    '--chart-4': '#8839ef',
    '--chart-5': '#D20F39',
    '--chart-actual': '#4C4F69',
    '--chart-reference': '#A6A8B6',
    '--chart-bank': '#40A02B',
    '--chart-crypto': '#F4600B',
    '--chart-property': '#1E66F5',
    '--chart-accent': '#8839ef',
    '--chart-income': '#40A02B',
    '--chart-over': '#D20F39',
    '--chart-grid': '#CBCFD8',
    '--chart-band': '#A6A8B659',
    '--chart-wash': '#4C4F691A',
    '--chart-heat-0': '#E3DBF4',
    '--chart-heat-1': '#CCB2F3',
    '--chart-heat-2': '#B58AF2',
    '--chart-heat-3': '#9F61F0',
    '--chart-heat-4': '#8839EF',
    '--money-income': '#29661C',
    '--money-expense': '#B30D30',
    '--money-transfer': '#1851C2',
    '--money-in-transit': '#7B4E10',
    '--radius': '0.75rem',
  },
}
