// check-ui-ignore raw-color: a built-in theme is token data, not UI
import type { Theme } from '../types'

/**
 * Ayu light: mode light, radius 0.5rem.
 * Upstream palette: Ayu (https://github.com/ayu-theme/ayu-colors), MIT licence, source https://github.com/ayu-theme/ayu-colors.
 * derived: --muted-foreground, #8A9199 (alpha)
 * tuned from upstream: --primary, #FA8532
 * tuned from upstream: --destructive, #F07171
 * tuned from upstream: --success, #6CBF43
 * tuned from upstream: --warning, #EBA400
 * tuned from upstream: --money-income, #6CBF43
 * tuned from upstream: --money-expense, #F07171
 * tuned from upstream: --money-transfer, #22A4E6
 * tuned from upstream: --money-in-transit, #EBA400
 * tuned from upstream: --chart-1, #22A4E6
 * tuned from upstream: --chart-2, #6CBF43
 * tuned from upstream: --chart-3, #FA8532
 * tuned from upstream: --chart-5, #F07171
 * tuned from upstream: --chart-bank, #6CBF43
 * tuned from upstream: --chart-crypto, #FA8532
 * tuned from upstream: --chart-property, #4CBF99
 * tuned from upstream: --chart-income, #6CBF43
 * tuned from upstream: --chart-over, #F07171
 */
export const ayuLight: Theme = {
  id: 'ayu-light',
  name: 'Ayu light',
  mode: 'light',
  tokens: {
    '--background': '#FCFCFC',
    '--foreground': '#5C6166',
    '--card': '#F3F4F5',
    '--card-foreground': '#5C6166',
    '--popover': '#F3F4F5',
    '--popover-foreground': '#5C6166',
    '--primary': '#AD622C',
    '--primary-foreground': '#FFFFFF',
    '--secondary': '#ECEEF0',
    '--secondary-foreground': '#5C6166',
    '--muted': '#ECEEF0',
    '--muted-foreground': '#666C73',
    '--accent': '#ECEEF0',
    '--accent-foreground': '#5C6166',
    '--destructive': '#A64E4E',
    '--destructive-foreground': '#FFFFFF',
    '--success': '#427529',
    '--success-foreground': '#FFFFFF',
    '--warning': '#886329',
    '--warning-foreground': '#FFFFFF',
    '--border': '#D8DADD',
    '--input': '#D8DADD',
    '--ring': '#AD622C',
    '--surface-inverse': '#5C6166',
    '--inverse-foreground': '#FCFCFC',
    '--success-soft': '#DBE4D6',
    '--warning-soft': '#E7E0D6',
    '--destructive-soft': '#EDDDDD',
    '--scrim': '#0000005C',
    '--text-tertiary': '#8A9199',
    '--border-strong': '#B3B6B9',
    '--sidebar': '#F3F4F5',
    '--sidebar-foreground': '#5C6166',
    '--sidebar-primary': '#AD622C',
    '--sidebar-primary-foreground': '#FFFFFF',
    '--sidebar-accent': '#ECEEF0',
    '--sidebar-accent-foreground': '#5C6166',
    '--sidebar-border': '#D8DADD',
    '--sidebar-ring': '#AD622C',
    '--chart-1': '#3592D4',
    '--chart-2': '#589C37',
    '--chart-3': '#CC7334',
    '--chart-4': '#A37ACC',
    '--chart-5': '#DD6868',
    '--chart-actual': '#5C6166',
    '--chart-reference': '#8A9199',
    '--chart-bank': '#589C37',
    '--chart-crypto': '#CC7334',
    '--chart-property': '#3D9C7D',
    '--chart-accent': '#A37ACC',
    '--chart-income': '#589C37',
    '--chart-over': '#DD6868',
    '--chart-grid': '#E3E4E7',
    '--money-income': '#427529',
    '--money-expense': '#A64E4E',
    '--money-transfer': '#276D9F',
    '--money-in-transit': '#886329',
    '--chart-band': '#8A919959',
    '--chart-wash': '#5C61661A',
    '--chart-heat-0': '#EBE2DD',
    '--chart-heat-1': '#DBC1AF',
    '--chart-heat-2': '#C99C7C',
    '--chart-heat-3': '#BB7F54',
    '--chart-heat-4': '#AD622C',
    '--radius': '0.5rem',
  },
}
