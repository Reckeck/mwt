// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Solarized dark: mode dark, radius 0.25rem.
 * Upstream palette: Solarized (https://ethanschoonover.com/solarized/), MIT licence, source https://github.com/altercation/solarized.
 * tuned from upstream: --money-income, #859900
 * tuned from upstream: --money-expense, #DC322F
 * tuned from upstream: --money-transfer, #268BD2
 * tuned from upstream: --money-in-transit, #B58900
 * tuned from upstream: --success, #859900
 * tuned from upstream: --destructive, #DC322F
 * tuned from upstream: --warning, #B58900
 * tuned from upstream: --primary-foreground, #002B36
 * tuned from upstream: --sidebar-primary-foreground, #002B36
 * tuned from upstream: --chart-3, #CB4B16
 * tuned from upstream: --chart-4, #6C71C4
 * tuned from upstream: --chart-5, #DC322F
 * tuned from upstream: --chart-crypto, #CB4B16
 * tuned from upstream: --chart-accent, #6C71C4
 * tuned from upstream: --chart-over, #DC322F
 */
export const solarizedDark: Theme = {
  id: 'solarized-dark',
  name: 'Solarized dark',
  mode: 'dark',
  tokens: {
    '--background': '#002B36',
    '--foreground': '#EEE8D5',
    '--card': '#073642',
    '--card-foreground': '#EEE8D5',
    '--popover': '#073642',
    '--popover-foreground': '#EEE8D5',
    '--primary': '#268BD2',
    '--primary-foreground': '#00222B',
    '--secondary': '#0F4552',
    '--secondary-foreground': '#EEE8D5',
    '--muted': '#0F4552',
    '--muted-foreground': '#93A1A1',
    '--accent': '#0F4552',
    '--accent-foreground': '#EEE8D5',
    '--destructive': '#EF908E',
    '--destructive-foreground': '#002B36',
    '--success': '#A2B23D',
    '--success-foreground': '#002B36',
    '--warning': '#C9A843',
    '--warning-foreground': '#002B36',
    '--border': '#21454C',
    '--input': '#21454C',
    '--ring': '#268BD2',
    '--surface-inverse': '#EEE8D5',
    '--inverse-foreground': '#002B36',
    '--success-soft': '#1B412B',
    '--warning-soft': '#243E2B',
    '--destructive-soft': '#2C2C35',
    '--scrim': '#000000A6',
    '--text-tertiary': '#60787C',
    '--border-strong': '#476466',
    '--sidebar': '#073642',
    '--sidebar-foreground': '#EEE8D5',
    '--sidebar-primary': '#268BD2',
    '--sidebar-primary-foreground': '#00222B',
    '--sidebar-accent': '#0F4552',
    '--sidebar-accent-foreground': '#EEE8D5',
    '--sidebar-border': '#21454C',
    '--sidebar-ring': '#268BD2',
    '--chart-1': '#268BD2',
    '--chart-2': '#859900',
    '--chart-3': '#CF5524',
    '--chart-4': '#7075C6',
    '--chart-5': '#DE3E3B',
    '--chart-actual': '#EEE8D5',
    '--chart-reference': '#5F7776',
    '--chart-bank': '#859900',
    '--chart-crypto': '#CF5524',
    '--chart-property': '#268BD2',
    '--chart-accent': '#7075C6',
    '--chart-income': '#859900',
    '--chart-over': '#DE3E3B',
    '--chart-grid': '#1D4249',
    '--money-income': '#A2B23D',
    '--money-expense': '#EF908E',
    '--money-transfer': '#72B2E0',
    '--money-in-transit': '#C9A843',
    '--chart-band': '#5F777659',
    '--chart-wash': '#EEE8D51A',
    '--chart-heat-0': '#033342',
    '--chart-heat-1': '#0A435D',
    '--chart-heat-2': '#135B84',
    '--chart-heat-3': '#1D73AB',
    '--chart-heat-4': '#268BD2',
    '--radius': '0.25rem',
  },
}
