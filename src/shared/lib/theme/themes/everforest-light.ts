// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Everforest light: mode light, radius 0.5rem.
 * Upstream palette: Everforest (https://github.com/sainnhe/everforest), MIT licence, source https://github.com/sainnhe/everforest.
 * tuned from upstream: --muted-foreground, #939F91
 * tuned from upstream: --primary, #8DA101
 * tuned from upstream: --destructive, #F85552
 * tuned from upstream: --success, #8DA101
 * tuned from upstream: --warning, #DFA000
 * tuned from upstream: --money-income, #8DA101
 * tuned from upstream: --money-expense, #F85552
 * tuned from upstream: --money-transfer, #3A94C5
 * tuned from upstream: --money-in-transit, #DFA000
 * tuned from upstream: --chart-1, #3A94C5
 * tuned from upstream: --chart-2, #8DA101
 * tuned from upstream: --chart-3, #F57D26
 * tuned from upstream: --chart-4, #DF69BA
 * tuned from upstream: --chart-5, #F85552
 * tuned from upstream: --chart-bank, #8DA101
 * tuned from upstream: --chart-crypto, #F57D26
 * tuned from upstream: --chart-property, #35A77C
 * tuned from upstream: --chart-accent, #DF69BA
 * tuned from upstream: --chart-income, #8DA101
 * tuned from upstream: --chart-over, #F85552
 */
export const everforestLight: Theme = {
  id: 'everforest-light',
  name: 'Everforest light',
  mode: 'light',
  tokens: {
    '--background': '#FDF6E3',
    '--foreground': '#5C6A72',
    '--card': '#F4F0D9',
    '--card-foreground': '#5C6A72',
    '--popover': '#F4F0D9',
    '--popover-foreground': '#5C6A72',
    '--primary': '#6A7A01',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#EFEBD4',
    '--secondary-foreground': '#5C6A72',
    '--muted': '#EFEBD4',
    '--muted-foreground': '#626C61',
    '--accent': '#EFEBD4',
    '--accent-foreground': '#5C6A72',
    '--destructive': '#BA403D',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#627001',
    '--success-foreground': '#FFFFFF',
    '--warning': '#886300',
    '--warning-foreground': '#FFFFFF',
    '--border': '#E0DCC7',
    '--input': '#E0DCC7',
    '--ring': '#6A7A01',
    '--surface-inverse': '#5C6A72',
    '--inverse-foreground': '#FDF6E3',
    '--success-soft': '#E1DEBA',
    '--warning-soft': '#E8DCBA',
    '--destructive-soft': '#F1D5C5',
    '--scrim': '#0000005C',
    '--text-tertiary': '#939F91',
    '--border-strong': '#B8BAAD',
    '--sidebar': '#F4F0D9',
    '--sidebar-foreground': '#5C6A72',
    '--sidebar-primary': '#6A7A01',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#EFEBD4',
    '--sidebar-accent-foreground': '#5C6A72',
    '--sidebar-border': '#E0DCC7',
    '--sidebar-ring': '#6A7A01',
    '--chart-1': '#388EBD',
    '--chart-2': '#7D8F01',
    '--chart-3': '#D06A21',
    '--chart-4': '#CD61AC',
    '--chart-5': '#EE524F',
    '--chart-actual': '#5C6A72',
    '--chart-reference': '#939F91',
    '--chart-bank': '#7D8F01',
    '--chart-crypto': '#D06A21',
    '--chart-property': '#319A72',
    '--chart-accent': '#CD61AC',
    '--chart-income': '#7D8F01',
    '--chart-over': '#EE524F',
    '--chart-grid': '#E8E4CE',
    '--money-income': '#627001',
    '--money-expense': '#BA403D',
    '--money-transfer': '#2C6F94',
    '--money-in-transit': '#886300',
    '--chart-band': '#939F9159',
    '--chart-wash': '#5C6A721A',
    '--chart-heat-0': '#E3E2BF',
    '--chart-heat-1': '#C4C78D',
    '--chart-heat-2': '#A1A957',
    '--chart-heat-3': '#86922C',
    '--chart-heat-4': '#6A7A01',
    '--radius': '0.5rem',
  },
}
