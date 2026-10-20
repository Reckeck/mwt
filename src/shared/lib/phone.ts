import { type Ref, ref } from 'vue'
import { useMediaQuery } from '@vueuse/core'

/**
 * A phone: narrower than Tailwind's `md` (768px). 767.98 so that at exactly
 * 768 the phone layout and the `md:` classes never apply together.
 */
export const PHONE_QUERY = '(max-width: 767.98px)'

/** Whether the window is phone-sized now; follows rotation and resizing. */
export function useIsPhone() {
  return useMediaQuery(PHONE_QUERY)
}

// check-ui-ignore raw-listener: one matchMedia listener for the app's lifetime, shared by every caller
let coarse: Ref<boolean> | null = null

/**
 * A finger, not a mouse (`pointer: coarse`): no hover and no right click, so
 * row actions show a «⋯» button. One media listener for the whole app — rows
 * ask for it a hundred times a page.
 */
export function useCoarsePointer(): Ref<boolean> {
  if (!coarse) {
    const query = typeof window === 'undefined' ? null : window.matchMedia('(pointer: coarse)')
    coarse = ref(query?.matches ?? false)
    query?.addEventListener('change', (event) => {
      if (coarse) coarse.value = event.matches
    })
  }
  return coarse
}
