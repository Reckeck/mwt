// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Alucard: mode light, radius 0.5rem.
 * Upstream palette: Alucard, the light variant of Dracula (https://draculatheme.com/spec), MIT licence, source https://github.com/dracula/dracula-theme.
 * tuned from upstream: --money-income, #14710A
 * tuned from upstream: --money-expense, #CB3A2A
 * tuned from upstream: --money-transfer, #036A96
 * tuned from upstream: --money-in-transit, #846E15
 * tuned from upstream: --success, #14710A
 * tuned from upstream: --destructive, #CB3A2A
 * tuned from upstream: --warning, #846E15
 */
export const alucard: Theme = {
  id: 'alucard',
  name: 'Alucard',
  mode: 'light',
  tokens: {
    '--background': '#FFFBEB',
    '--foreground': '#1F1F1F',
    '--card': '#FFFFFF',
    '--card-foreground': '#1F1F1F',
    '--popover': '#FFFFFF',
    '--popover-foreground': '#1F1F1F',
    '--primary': '#644AC9',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#CFCFDE',
    '--secondary-foreground': '#1F1F1F',
    '--muted': '#CFCFDE',
    '--muted-foreground': '#6C664B',
    '--accent': '#CFCFDE',
    '--accent-foreground': '#1F1F1F',
    '--destructive': '#A42F23',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#11680A',
    '--success-foreground': '#FFFFFF',
    '--warning': '#685610',
    '--warning-foreground': '#FFFFFF',
    '--border': '#E0DCCE',
    '--input': '#E0DCCE',
    '--ring': '#644AC9',
    '--surface-inverse': '#1F1F1F',
    '--inverse-foreground': '#FFFBEB',
    '--success-soft': '#D9E5C7',
    '--warning-soft': '#EBE4C9',
    '--destructive-soft': '#F7DCCC',
    '--scrim': '#0000005C',
    '--text-tertiary': '#9F9A83',
    '--border-strong': '#BCB9AE',
    '--sidebar': '#FFFFFF',
    '--sidebar-foreground': '#1F1F1F',
    '--sidebar-primary': '#644AC9',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#CFCFDE',
    '--sidebar-accent-foreground': '#1F1F1F',
    '--sidebar-border': '#E0DCCE',
    '--sidebar-ring': '#644AC9',
    '--chart-1': '#036A96',
    '--chart-2': '#14710A',
    '--chart-3': '#A34D14',
    '--chart-4': '#644AC9',
    '--chart-5': '#CB3A2A',
    '--chart-actual': '#1F1F1F',
    '--chart-reference': '#A5A399',
    '--chart-bank': '#14710A',
    '--chart-crypto': '#A34D14',
    '--chart-property': '#036A96',
    '--chart-accent': '#644AC9',
    '--chart-income': '#14710A',
    '--chart-over': '#CB3A2A',
    '--chart-grid': '#E4E1D3',
    '--money-income': '#11680A',
    '--money-expense': '#A42F23',
    '--money-transfer': '#035E86',
    '--money-in-transit': '#685610',
    '--chart-band': '#A5A39959',
    '--chart-wash': '#1F1F1F1A',
    '--chart-heat-0': '#F3EDE8',
    '--chart-heat-1': '#D8CFE3',
    '--chart-heat-2': '#B2A3DA',
    '--chart-heat-3': '#8B76D2',
    '--chart-heat-4': '#644AC9',
    '--radius': '0.5rem',
  },
}
