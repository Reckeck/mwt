import { onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const SALARY_PRESET: PresetInfo = { preset: 'salary', tx_kind: 'topup', category: 'salary' }

/**
 * `?new=salary` (from the daily budget or the onboarding checklist) opens the
 * salary preset at once, then drops the parameter so a reload does not reopen it.
 */
export function useNewFromRoute(enabled: () => boolean, presets: () => PresetInfo[], open: (from: PresetInfo) => void) {
  const route = useRoute()
  const router = useRouter()
  onMounted(() => {
    if (!enabled() || route.query.new !== 'salary') return
    open(presets().find((entry) => entry.preset === 'salary') ?? SALARY_PRESET)
    const { new: _new, ...rest } = route.query
    void router.replace({ query: rest })
  })
}

/**
 * `?edit=<id>` (from the editor of an automation's workflow) opens that
 * automation's form once the list is in, then drops the parameter.
 */
export function useEditFromRoute(list: () => Automation[] | undefined, open: (automation: Automation) => void) {
  const route = useRoute()
  const router = useRouter()
  const stop = watch(
    list,
    (automations) => {
      const id = route.query.edit
      if (typeof id !== 'string' || !automations) return
      const found = automations.find((entry) => entry.id === id)
      if (found) open(found)
      const { edit: _edit, ...rest } = route.query
      void router.replace({ query: rest })
      queueMicrotask(() => stop())
    },
    { immediate: true },
  )
}
