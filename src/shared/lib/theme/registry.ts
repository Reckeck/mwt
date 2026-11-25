import { alucard } from './themes/alucard'
import { defaultDark } from './themes/default-dark'
import { defaultLight } from './themes/default-light'
import { dracula } from './themes/dracula'
import { nord } from './themes/nord'
import { nordSnowStorm } from './themes/nord-snow-storm'
import { solarizedDark } from './themes/solarized-dark'
import { solarizedLight } from './themes/solarized-light'
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
]

export function getTheme(id: string): Theme | undefined {
  return builtinThemes.find((theme) => theme.id === id)
}
