import { type MaybeRefOrGetter, nextTick, toValue, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

/** The query key a link sets to point at one row of a list (docs/ui-links.md, 15). */
export const FOCUS_QUERY_KEY = 'focus'
/** The mark a row carries: `data-focus-row="<id>"`. */
export const FOCUS_ATTR = 'data-focus-row'
const FLASH_CLASSES = ['bg-accent', 'ring-2', 'ring-ring']
const FLASH_MS = 2200

/**
 * A list scrolls to the row named by `?focus=<id>` and lights it for a moment,
 * then drops the key from the address. Rows carry `data-focus-row`. A list that
 * does not hold the id leaves the key for a sibling list on the same page.
 */
export function useFocusRow(ready: MaybeRefOrGetter<boolean>) {
  const route = useRoute()
  const router = useRouter()
  let timer: ReturnType<typeof setTimeout> | undefined

  watch(
    () => [route.query[FOCUS_QUERY_KEY], toValue(ready)] as const,
    async ([focus, isReady]) => {
      if (typeof focus !== 'string' || !focus || !isReady) return
      await nextTick()
      const row = [...document.querySelectorAll<HTMLElement>(`[${FOCUS_ATTR}]`)].find(
        (node) => node.getAttribute(FOCUS_ATTR) === focus,
      )
      if (!row) return
      row.scrollIntoView({ block: 'center', behavior: 'smooth' })
      row.classList.add(...FLASH_CLASSES)
      clearTimeout(timer)
      timer = setTimeout(() => row.classList.remove(...FLASH_CLASSES), FLASH_MS)
      const { [FOCUS_QUERY_KEY]: _focus, ...rest } = route.query
      void router.replace({ query: rest })
    },
    { immediate: true, flush: 'post' },
  )
}
