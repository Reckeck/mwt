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
import { everforestDark } from './themes/everforest-dark'
import { everforestLight } from './themes/everforest-light'
import { kanagawaWave } from './themes/kanagawa-wave'
import { kanagawaLotus } from './themes/kanagawa-lotus'
import { githubDark } from './themes/github-dark'
import { githubLight } from './themes/github-light'
import { ayuMirage } from './themes/ayu-mirage'
import { ayuLight } from './themes/ayu-light'
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
  everforestDark,
  everforestLight,
  kanagawaWave,
  kanagawaLotus,
  githubDark,
  githubLight,
  ayuMirage,
  ayuLight,
]

export function getTheme(id: string): Theme | undefined {
  return builtinThemes.find((theme) => theme.id === id)
}
