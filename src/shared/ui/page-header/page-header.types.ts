import type { RouteLocationRaw } from 'vue-router'

export interface PageCrumb {
  label: string
  /** Where the crumb leads; the last crumb (the current page) has none. */
  to?: RouteLocationRaw
}

export interface PageHeaderProps {
  /** The page's own title; omit when the shell's title already says it. */
  title?: string
  /** One muted line under the title. */
  description?: string
  /** The path to this page; the shell title is the root, so start below it. */
  crumbs?: PageCrumb[]
}
