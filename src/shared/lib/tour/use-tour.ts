import { driver, type Driver } from 'driver.js'
import 'driver.js/dist/driver.css'
import { useI18n } from 'vue-i18n'

import { pushBackLayer } from '@/shared/lib/back-stack'
import { useSettingsStore } from '@/shared/store/settings'

import { TOURS, WORKFLOW_EDITOR_EMPTY } from './tours'

/** The tour on screen: one at a time, gone when the page changes. */
let active: Driver | null = null
/** Set while a tour is closed because the page changed: that is not «done». */
let leaving = false

/**
 * Whether a step's element is on the page for real: a hidden one (a phone
 * layout, a closed panel) would leave the popover pointing at nothing.
 */
function isShown(element: HTMLElement): boolean {
  return element.checkVisibility?.() !== false && element.getClientRects().length > 0
}

/** Guided tours over `[data-tour]` elements; a finished tour is remembered in the app settings. */
export function useTour() {
  const { t } = useI18n()
  const settings = useSettingsStore()

  function hasTour(id: string): boolean {
    return Boolean(TOURS[id]?.length)
  }

  function start(id: string): boolean {
    // A new workflow with nothing on the canvas gets the short tour.
    const empty = id === 'workflow-new' && document.querySelector('[data-tour="workflow-add-first"]') !== null
    const steps = (empty ? WORKFLOW_EDITOR_EMPTY : (TOURS[id] ?? []))
      .map((step) => ({ step, element: document.querySelector<HTMLElement>(`[data-tour="${step.target}"]`) }))
      .filter(
        (entry): entry is { step: (typeof entry)['step']; element: HTMLElement } =>
          entry.element !== null && isShown(entry.element),
      )
    if (!steps.length) return false
    stop()
    let release = () => {}
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
        // driver.js scrolls the window only: a target inside a scrolled list
        // (the sidebar) would be lit where it is clipped away.
        onHighlightStarted: () => element.scrollIntoView({ block: 'nearest' }),
        popover: {
          title: t(`tour.${step.key}.title`),
          description: t(`tour.${step.key}.body`),
          side: step.side,
        },
      })),
      onDestroyed: () => {
        release()
        if (active === tour) active = null
        if (!leaving) void markDone(id)
      },
    })
    active = tour
    // «Back» closes the tour like its close button.
    release = pushBackLayer(() => tour.destroy())
    tour.drive()
    return true
  }

  /** Close the tour on screen without counting it as seen (the page changed). */
  function stop() {
    if (!active) return
    leaving = true
    try {
      active.destroy()
    } finally {
      leaving = false
      active = null
    }
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

  function isDone(id: string): boolean {
    return Boolean(settings.app?.completed_tours.includes(id))
  }

  return { hasTour, start, stop, autoStart, resetAll, isDone }
}
