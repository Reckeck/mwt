import { useI18n } from 'vue-i18n'
import { type RouteLocationRaw, useRouter } from 'vue-router'

/**
 * The way back for a screen that can be reached from several places (the
 * workflow editor): where the user came from inside the app, else `fallback`.
 * `label` names that place by its section; `go` steps there.
 */
export function useCameFrom(fallback: RouteLocationRaw, fallbackLabelKey: string) {
  const router = useRouter()
  const { t } = useI18n()
  const previous = typeof window.history.state?.back === 'string' ? (window.history.state.back as string) : null
  const known = previous ? router.resolve(previous) : null
  const internal = Boolean(known?.matched.length) && !known?.matched.some((record) => record.path.includes('pathMatch'))
  const key = internal && typeof known?.meta.i18n === 'string' ? known.meta.i18n : fallbackLabelKey

  function go() {
    if (internal) router.back()
    else void router.push(fallback)
  }
  return { label: t(key), go }
}
