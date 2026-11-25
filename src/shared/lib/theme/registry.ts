import { defaultDark } from './themes/default-dark'
import { defaultLight } from './themes/default-light'
import type { Theme } from './types'

export const builtinThemes: readonly Theme[] = [defaultLight, defaultDark]

export function getTheme(id: string): Theme | undefined {
  return builtinThemes.find((theme) => theme.id === id)
}
