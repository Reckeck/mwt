// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Tokyo Night Day: mode light, radius 0.5rem.
 * Upstream palette: Tokyo Night Day (https://github.com/folke/tokyonight.nvim), Apache-2.0 licence (the light variant exists only there), source https://github.com/folke/tokyonight.nvim.
 * tuned from upstream: --secondary-foreground, #3760BF
 * tuned from upstream: --muted-foreground, #6172B0
 * tuned from upstream: --accent-foreground, #3760BF
 * tuned from upstream: --destructive, #F52A65
 * tuned from upstream: --destructive-foreground, #151515
 * tuned from upstream: --success, #587539
 * tuned from upstream: --warning, #8C6C3E
 * tuned from upstream: --sidebar-accent-foreground, #3760BF
 * tuned from upstream: --money-income, #587539
 * tuned from upstream: --money-expense, #F52A65
 * tuned from upstream: --money-transfer, #2E7DE9
 * tuned from upstream: --money-in-transit, #8C6C3E
 */
export const tokyoNightDay: Theme = {
  id: 'tokyo-night-day',
  name: 'Tokyo Night Day',
  mode: 'light',
  tokens: {
    '--background': '#E1E2E7',
    '--foreground': '#3760BF',
    '--card': '#EDEEF3',
    '--card-foreground': '#3760BF',
    '--popover': '#EDEEF3',
    '--popover-foreground': '#3760BF',
    '--primary': '#2E7DE9',
    '--primary-foreground': '#151515',
    '--secondary': '#D0D5E3',
    '--secondary-foreground': '#3157AE',
    '--muted': '#D0D5E3',
    '--muted-foreground': '#526397',
    '--accent': '#D0D5E3',
    '--accent-foreground': '#3157AE',
    '--destructive': '#AF1F49',
    '--destructive-foreground': '#D3D3D3',
    '--success': '#4A622F',
    '--success-foreground': '#FFFFFF',
    '--warning': '#715731',
    '--warning-foreground': '#FFFFFF',
    '--border': '#C4C8DA',
    '--input': '#C4C8DA',
    '--ring': '#2E7DE9',
    '--surface-inverse': '#3760BF',
    '--inverse-foreground': '#E1E2E7',
    '--success-soft': '#D8DDD9',
    '--warning-soft': '#DFDCDA',
    '--destructive-soft': '#EED3DF',
    '--scrim': '#0000005C',
    '--text-tertiary': '#7B94CF',
    '--border-strong': '#9AA9D2',
    '--sidebar': '#EDEEF3',
    '--sidebar-foreground': '#3760BF',
    '--sidebar-primary': '#2E7DE9',
    '--sidebar-primary-foreground': '#151515',
    '--sidebar-accent': '#D0D5E3',
    '--sidebar-accent-foreground': '#3157AE',
    '--sidebar-border': '#C4C8DA',
    '--sidebar-ring': '#2E7DE9',
    '--chart-1': '#2E7DE9',
    '--chart-2': '#587539',
    '--chart-3': '#B15C00',
    '--chart-4': '#9854F1',
    '--chart-5': '#F52A65',
    '--chart-actual': '#3760BF',
    '--chart-reference': '#8CA1D3',
    '--chart-bank': '#587539',
    '--chart-crypto': '#B15C00',
    '--chart-property': '#2E7DE9',
    '--chart-accent': '#9854F1',
    '--chart-income': '#587539',
    '--chart-over': '#F52A65',
    '--chart-grid': '#D7DDED',
    '--chart-band': '#8CA1D359',
    '--chart-wash': '#3760BF1A',
    '--chart-heat-0': '#D6E0F2',
    '--chart-heat-1': '#B4CCF0',
    '--chart-heat-2': '#8EB6EE',
    '--chart-heat-3': '#5E99EC',
    '--chart-heat-4': '#2E7DE9',
    '--money-income': '#4A622F',
    '--money-expense': '#AF1F49',
    '--money-transfer': '#2359A7',
    '--money-in-transit': '#715731',
    '--radius': '0.5rem',
  },
}
