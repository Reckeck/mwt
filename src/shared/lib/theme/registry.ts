import type { Theme } from './types'

export const builtinThemes: readonly Theme[] = []

export function getTheme(id: string): Theme | undefined {
  return builtinThemes.find((theme) => theme.id === id)
}
