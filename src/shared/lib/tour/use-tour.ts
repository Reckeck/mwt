import { driver } from 'driver.js'
import 'driver.js/dist/driver.css'
import { useI18n } from 'vue-i18n'

import { useSettingsStore } from '@/shared/store/settings'

import { TOURS } from './tours'

/** Guided tours over `[data-tour]` elements; a finished tour is remembered in the app settings. */
export function useTour() {
  const { t } = useI18n()
  const settings = useSettingsStore()

  function hasTour(id: string): boolean {
    return Boolean(TOURS[id]?.length)
  }

  function start(id: string): boolean {
    const steps = (TOURS[id] ?? [])
      .map((step) => ({ step, element: document.querySelector<HTMLElement>(`[data-tour="${step.target}"]`) }))
      .filter((entry): entry is { step: (typeof entry)['step']; element: HTMLElement } => entry.element !== null)
    if (!steps.length) return false
    const tour = driver({
      showProgress: steps.length > 1,
      // driver.js fills its own {{current}} / {{total}} tokens in.
      progressText: t('tour.progress', { current: '{{current}}', total: '{{total}}' }),
      nextBtnText: t('tour.next'),
      prevBtnText: t('tour.prev'),
      doneBtnText: t('tour.done'),
      allowClose: true,
      steps: steps.map(({ step, element }) => ({
        element,
        popover: {
          title: t(`tour.${step.key}.title`),
          description: t(`tour.${step.key}.body`),
          side: step.side,
        },
      })),
      onDestroyed: () => void markDone(id),
    })
    tour.drive()
    return true
  }

  async function markDone(id: string) {
    const done = settings.app?.completed_tours ?? []
    if (done.includes(id)) return
    try {
      await settings.updateApp({ completed_tours: [...done, id] })
    } catch {
      // Not remembered: the tour simply offers itself again next time.
    }
  }

  /** First visit of a page with a tour: start it once. */
  function autoStart(id: string) {
    if (!hasTour(id) || !settings.app || settings.app.completed_tours.includes(id)) return
    start(id)
  }

  async function resetAll() {
    await settings.updateApp({ completed_tours: [] })
  }

  return { hasTour, start, autoStart, resetAll }
}
