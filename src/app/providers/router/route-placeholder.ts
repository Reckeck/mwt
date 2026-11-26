/** Every screen is rebuilt in W4 (spec D14): until then each route mounts the same placeholder. */
export const routePlaceholder = () => import('@/pages/route-placeholder').then((page) => page.RoutePlaceholder)
