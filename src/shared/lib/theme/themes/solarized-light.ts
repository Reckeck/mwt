// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Solarized light: mode light, radius 0.25rem.
 * Upstream palette: Solarized (https://ethanschoonover.com/solarized/), MIT licence, source https://github.com/altercation/solarized.
 * tuned from upstream: --money-income, #859900
 * tuned from upstream: --money-expense, #DC322F
 * tuned from upstream: --money-transfer, #268BD2
 * tuned from upstream: --money-in-transit, #B58900
 * tuned from upstream: --success, #859900
 * tuned from upstream: --destructive, #DC322F
 * tuned from upstream: --warning, #B58900
 * tuned from upstream: --primary-foreground, #FFFFFF
 * tuned from upstream: --sidebar-primary-foreground, #FFFFFF
 */
export const solarizedLight: Theme = {
  id: 'solarized-light',
  name: 'Solarized light',
  mode: 'light',
  tokens: {
    '--background': '#FDF6E3',
    '--foreground': '#073642',
    '--card': '#FFFCF2',
    '--card-foreground': '#073642',
    '--popover': '#FFFCF2',
    '--popover-foreground': '#073642',
    '--primary': '#268BD2',
    '--primary-foreground': '#1E1E1E',
    '--secondary': '#EEE8D5',
    '--secondary-foreground': '#073642',
    '--muted': '#EEE8D5',
    '--muted-foreground': '#586E75',
    '--accent': '#EEE8D5',
    '--accent-foreground': '#073642',
    '--destructive': '#C32E2B',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#5F6E00',
    '--success-foreground': '#FFFFFF',
    '--warning': '#826200',
    '--warning-foreground': '#FFFFFF',
    '--border': '#DBDBCC',
    '--input': '#DBDBCC',
    '--ring': '#268BD2',
    '--surface-inverse': '#073642',
    '--inverse-foreground': '#FDF6E3',
    '--success-soft': '#EAE7BF',
    '--warning-soft': '#F1E5BF',
    '--destructive-soft': '#F8D7C6',
    '--scrim': '#0000005C',
    '--text-tertiary': '#929E9C',
    '--border-strong': '#B3BCB3',
    '--sidebar': '#FFFCF2',
    '--sidebar-foreground': '#073642',
    '--sidebar-primary': '#268BD2',
    '--sidebar-primary-foreground': '#1E1E1E',
    '--sidebar-accent': '#EEE8D5',
    '--sidebar-accent-foreground': '#073642',
    '--sidebar-border': '#DBDBCC',
    '--sidebar-ring': '#268BD2',
    '--chart-1': '#268BD2',
    '--chart-2': '#859900',
    '--chart-3': '#CB4B16',
    '--chart-4': '#6C71C4',
    '--chart-5': '#DC322F',
    '--chart-actual': '#073642',
    '--chart-reference': '#9BA9A3',
    '--chart-bank': '#859900',
    '--chart-crypto': '#CB4B16',
    '--chart-property': '#268BD2',
    '--chart-accent': '#6C71C4',
    '--chart-income': '#859900',
    '--chart-over': '#DC322F',
    '--chart-grid': '#DFDFD0',
    '--money-income': '#5F6E00',
    '--money-expense': '#C32E2B',
    '--money-transfer': '#1E6DA5',
    '--money-in-transit': '#826200',
    '--chart-band': '#9BA9A359',
    '--chart-wash': '#0736421A',
    '--chart-heat-0': '#ECEDE2',
    '--chart-heat-1': '#C7DBDF',
    '--chart-heat-2': '#92C1DB',
    '--chart-heat-3': '#5CA6D6',
    '--chart-heat-4': '#268BD2',
    '--radius': '0.25rem',
  },
}
