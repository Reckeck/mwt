import { onMounted } from 'vue'
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
