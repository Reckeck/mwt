import { computed, type ComputedRef } from 'vue'

/** One axis/label colour that reads on both themes. */
export function chartAxisColor(resolvedTheme: ComputedRef<'light' | 'dark'>): ComputedRef<string> {
  return computed(() => (resolvedTheme.value === 'dark' ? 'rgba(255,255,255,0.45)' : 'rgba(0,0,0,0.45)'))
}
