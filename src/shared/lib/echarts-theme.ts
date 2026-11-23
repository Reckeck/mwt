import { computed, type ComputedRef } from 'vue'

import { chartColors } from './chart-theme'

/** One axis/label colour (`--muted-foreground`) that reads on both themes; re-read when the theme flips. */
export function chartAxisColor(resolvedTheme: ComputedRef<'light' | 'dark'>): ComputedRef<string> {
  return computed(() => {
    void resolvedTheme.value
    return chartColors().muted
  })
}
