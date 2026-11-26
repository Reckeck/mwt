// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Gruvbox light: mode light, radius 0.25rem.
 * Upstream palette: Gruvbox (https://github.com/morhetz/gruvbox), MIT licence, source https://github.com/morhetz/gruvbox.
 * tuned from upstream: --primary-foreground, #FBF1C7
 * tuned from upstream: --muted-foreground, #7C6F64
 * tuned from upstream: --destructive-foreground, #FBF1C7
 * tuned from upstream: --success, #79740E
 * tuned from upstream: --success-foreground, #FBF1C7
 * tuned from upstream: --warning, #B57614
 * tuned from upstream: --warning-foreground, #FBF1C7
 * tuned from upstream: --money-income, #79740E
 * tuned from upstream: --money-in-transit, #B57614
 */
export const gruvboxLight: Theme = {
  id: 'gruvbox-light',
  name: 'Gruvbox light',
  mode: 'light',
  tokens: {
    '--background': '#FBF1C7',
    '--foreground': '#3C3836',
    '--card': '#F9F5D7',
    '--card-foreground': '#3C3836',
    '--popover': '#F9F5D7',
    '--popover-foreground': '#3C3836',
    '--primary': '#076678',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#EBDBB2',
    '--secondary-foreground': '#3C3836',
    '--muted': '#EBDBB2',
    '--muted-foreground': '#776B60',
    '--accent': '#EBDBB2',
    '--accent-foreground': '#3C3836',
    '--destructive': '#9D0006',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#67630C',
    '--success-foreground': '#FFFFFF',
    '--warning': '#86570F',
    '--warning-foreground': '#FFFFFF',
    '--border': '#D5C4A1',
    '--input': '#D5C4A1',
    '--ring': '#076678',
    '--surface-inverse': '#3C3836',
    '--inverse-foreground': '#FBF1C7',
    '--success-soft': '#E8DEAB',
    '--warning-soft': '#F1DFAC',
    '--destructive-soft': '#EDCDAA',
    '--scrim': '#0000005C',
    '--text-tertiary': '#928B77',
    '--border-strong': '#9F937C',
    '--sidebar': '#F9F5D7',
    '--sidebar-foreground': '#3C3836',
    '--sidebar-primary': '#076678',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#EBDBB2',
    '--sidebar-accent-foreground': '#3C3836',
    '--sidebar-border': '#D5C4A1',
    '--sidebar-ring': '#076678',
    '--chart-1': '#076678',
    '--chart-2': '#79740E',
    '--chart-3': '#AF3A03',
    '--chart-4': '#8F3F71',
    '--chart-5': '#9D0006',
    '--chart-actual': '#3C3836',
    '--chart-reference': '#A59E86',
    '--chart-bank': '#79740E',
    '--chart-crypto': '#AF3A03',
    '--chart-property': '#076678',
    '--chart-accent': '#8F3F71',
    '--chart-income': '#79740E',
    '--chart-over': '#9D0006',
    '--chart-grid': '#E0D2AC',
    '--chart-band': '#A59E8659',
    '--chart-wash': '#3C38361A',
    '--chart-heat-0': '#DEE0BE',
    '--chart-heat-1': '#A8C2AC',
    '--chart-heat-2': '#72A39B',
    '--chart-heat-3': '#3D8589',
    '--chart-heat-4': '#076678',
    '--money-income': '#67630C',
    '--money-expense': '#9D0006',
    '--money-transfer': '#076678',
    '--money-in-transit': '#86570F',
    '--radius': '0.25rem',
  },
}
