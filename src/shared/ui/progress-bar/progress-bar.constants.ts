import type { ProgressSize, ProgressTone } from './progress-bar.types'

export const PROGRESS_TONE_CLASS: Record<ProgressTone, string> = {
  primary: 'bg-primary',
  destructive: 'bg-destructive',
  warning: 'bg-warning',
}

export const PROGRESS_SIZE_CLASS: Record<ProgressSize, string> = { sm: 'h-1.5', md: 'h-2' }
