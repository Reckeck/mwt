import { alucard } from './themes/alucard'
import { catppuccinFrappe } from './themes/catppuccin-frappe'
import { catppuccinLatte } from './themes/catppuccin-latte'
import { catppuccinMacchiato } from './themes/catppuccin-macchiato'
import { catppuccinMocha } from './themes/catppuccin-mocha'
import { gruvboxDark } from './themes/gruvbox-dark'
import { gruvboxLight } from './themes/gruvbox-light'
import { defaultDark } from './themes/default-dark'
import { defaultLight } from './themes/default-light'
import { dracula } from './themes/dracula'
import { nord } from './themes/nord'
import { nordSnowStorm } from './themes/nord-snow-storm'
import { solarizedDark } from './themes/solarized-dark'
import { solarizedLight } from './themes/solarized-light'
import { oneDark } from './themes/one-dark'
import { oneLight } from './themes/one-light'
import { rosePine } from './themes/rose-pine'
import { rosePineDawn } from './themes/rose-pine-dawn'
import { rosePineMoon } from './themes/rose-pine-moon'
import { tokyoNight } from './themes/tokyo-night'
import { tokyoNightDay } from './themes/tokyo-night-day'
import { tokyoNightStorm } from './themes/tokyo-night-storm'
import type { Theme } from './types'

export const builtinThemes: readonly Theme[] = [
  defaultLight,
  defaultDark,
  dracula,
  alucard,
  nord,
  nordSnowStorm,
  solarizedDark,
  solarizedLight,
  gruvboxDark,
  gruvboxLight,
  catppuccinMocha,
  catppuccinMacchiato,
  catppuccinFrappe,
  catppuccinLatte,
  tokyoNight,
  tokyoNightStorm,
  tokyoNightDay,
  oneDark,
  oneLight,
  rosePine,
  rosePineMoon,
  rosePineDawn,
]

export function getTheme(id: string): Theme | undefined {
  return builtinThemes.find((theme) => theme.id === id)
}
