import { NAV_SECTIONS } from './nav-sections'

/**
 * The phone top bar owns Back: it shows off a section root and nowhere else; pages draw no Back
 * of their own on a phone.
 */
export function showBack(phone: boolean, routeName: unknown): boolean {
  return phone && !NAV_SECTIONS.some((section) => section.root === routeName)
}
