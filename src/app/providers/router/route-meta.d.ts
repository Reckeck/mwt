import 'vue-router'

declare module 'vue-router' {
  interface RouteMeta {
    /** Open before the vault is unlocked. */
    public?: boolean
    /** i18n key of the screen's title in the top bar. */
    i18n?: string
    /** Analytics section the route belongs to. */
    group?: string
    /** On a phone the screen takes the whole window: no top bar, no bottom navigation. */
    fullscreen?: boolean
  }
}
