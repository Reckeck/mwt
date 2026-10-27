/**
 * Where the toasts sit: above the phone's bottom bar, and above the footer of the layer
 * that is open (a sheet's or a dialog's «Удалить / Изменить»), so a toast never covers
 * the buttons it comes from. Written to `--toast-bottom` on `<html>`; styles.css puts the toaster there.
 */
const GAP = 12
const DEFAULT = 16

function clearance(): number {
  const height = window.innerHeight
  const layers = [...document.querySelectorAll<HTMLElement>('[role="dialog"], [role="alertdialog"]')].filter(
    (layer) => layer.getClientRects().length,
  )
  const layer = layers.at(-1)
  const footer = layer
    ? [...layer.querySelectorAll<HTMLElement>('footer, [data-slot$="-footer"]')]
        .filter((node) => node.getClientRects().length)
        .at(-1)
    : undefined
  if (footer) {
    const top = footer.getBoundingClientRect().top
    // A footer that is not at the window's bottom (a dialog in the middle) leaves the toast in its corner.
    if (top > height * 0.4 && footer.getBoundingClientRect().bottom >= height - 4) return Math.round(height - top + GAP)
  }
  const bar = document.querySelector<HTMLElement>('[data-tour="bottom-nav"]')
  if (bar?.getClientRects().length) return Math.round(height - bar.getBoundingClientRect().top + GAP)
  return DEFAULT
}

/** Keeps `--toast-bottom` current while layers open, close and slide; returns the stop function. */
export function installToastClearance(): () => void {
  const root = document.documentElement
  let frame = 0
  const apply = () => {
    frame = 0
    root.style.setProperty('--toast-bottom', `${clearance()}px`)
  }
  const schedule = () => {
    // Twice: a sheet slides in for ~300 ms, its footer has settled by the second pass.
    if (!frame) frame = requestAnimationFrame(apply)
    window.setTimeout(apply, 350)
  }
  const observer = new MutationObserver(schedule)
  observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-state'] })
  window.addEventListener('resize', schedule)
  schedule()
  return () => {
    observer.disconnect()
    window.removeEventListener('resize', schedule)
    if (frame) cancelAnimationFrame(frame)
    root.style.removeProperty('--toast-bottom')
  }
}
