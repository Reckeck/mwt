import type { ThemeToken } from './tokens'

export type ThemeMode = 'light' | 'dark'

export type ThemeTokens = Record<ThemeToken, string>

export interface Theme {
  id: string
  name: string
  mode: ThemeMode
  tokens: ThemeTokens
}
