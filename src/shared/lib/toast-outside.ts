/**
 * A toast sits outside every dialog, yet clicking its «Undo» must not close
 * the dialog under it. Overlays pass this to `@pointer-down-outside` and
 * `@interact-outside`; the toaster itself gets `pointer-events: auto` in
 * styles.css (Reka turns them off on the body while a modal is open).
 */
export function keepOpenOnToast(event: CustomEvent<{ originalEvent?: Event }>) {
  const target = event.detail?.originalEvent?.target ?? event.target
  if (target instanceof Element && target.closest('[data-sonner-toaster], .driver-popover, .driver-overlay'))
    event.preventDefault()
}
