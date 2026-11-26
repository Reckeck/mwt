// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * One Light: mode light, radius 0.375rem.
 * Upstream palette: One Light (Atom One Light syntax colours), MIT licence, source https://github.com/atom/atom/tree/master/packages/one-light-syntax.
 * tuned from upstream: --destructive, #E45649
 * tuned from upstream: --destructive-foreground, #151515
 * tuned from upstream: --success, #50A14F
 * tuned from upstream: --success-foreground, #151515
 * tuned from upstream: --warning, #C18401
 * tuned from upstream: --money-income, #50A14F
 * tuned from upstream: --money-expense, #E45649
 * tuned from upstream: --money-transfer, #4078F2
 * tuned from upstream: --money-in-transit, #C18401
 */
export const oneLight: Theme = {
  id: 'one-light',
  name: 'One Light',
  mode: 'light',
  tokens: {
    '--background': '#FAFAFA',
    '--foreground': '#383A42',
    '--card': '#FFFFFF',
    '--card-foreground': '#383A42',
    '--popover': '#FFFFFF',
    '--popover-foreground': '#383A42',
    '--primary': '#4078F2',
    '--primary-foreground': '#151515',
    '--secondary': '#F0F0F1',
    '--secondary-foreground': '#383A42',
    '--muted': '#F0F0F1',
    '--muted-foreground': '#696C77',
    '--accent': '#F0F0F1',
    '--accent-foreground': '#383A42',
    '--destructive': '#B8473B',
    '--destructive-foreground': '#EEEEEE',
    '--success': '#3E7A3D',
    '--success-foreground': '#EFEFEF',
    '--warning': '#926501',
    '--warning-foreground': '#151515',
    '--border': '#DBDBDC',
    '--input': '#DBDBDC',
    '--ring': '#4078F2',
    '--surface-inverse': '#383A42',
    '--inverse-foreground': '#FAFAFA',
    '--success-soft': '#E7F2E6',
    '--warning-soft': '#F6EEDB',
    '--destructive-soft': '#FBE7E6',
    '--scrim': '#0000005C',
    '--text-tertiary': '#86878C',
    '--border-strong': '#AAABAE',
    '--sidebar': '#FFFFFF',
    '--sidebar-foreground': '#383A42',
    '--sidebar-primary': '#4078F2',
    '--sidebar-primary-foreground': '#151515',
    '--sidebar-accent': '#F0F0F1',
    '--sidebar-accent-foreground': '#383A42',
    '--sidebar-border': '#DBDBDC',
    '--sidebar-ring': '#4078F2',
    '--chart-1': '#4078F2',
    '--chart-2': '#50A14F',
    '--chart-3': '#986801',
    '--chart-4': '#A626A4',
    '--chart-5': '#E45649',
    '--chart-actual': '#383A42',
    '--chart-reference': '#999A9E',
    '--chart-bank': '#50A14F',
    '--chart-crypto': '#986801',
    '--chart-property': '#4078F2',
    '--chart-accent': '#A626A4',
    '--chart-income': '#50A14F',
    '--chart-over': '#E45649',
    '--chart-grid': '#E7E7E8',
    '--chart-band': '#999A9E59',
    '--chart-wash': '#383A421A',
    '--chart-heat-0': '#E8EFFD',
    '--chart-heat-1': '#C6D7FB',
    '--chart-heat-2': '#A0BCF9',
    '--chart-heat-3': '#709AF5',
    '--chart-heat-4': '#4078F2',
    '--money-income': '#3E7A3D',
    '--money-expense': '#B8473B',
    '--money-transfer': '#3668D0',
    '--money-in-transit': '#926501',
    '--radius': '0.375rem',
  },
}
