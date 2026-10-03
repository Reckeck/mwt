// Unified spacing tokens for TradeApp
// Base unit = 4px (Tailwind's 1 unit = 0.25rem = 4px)
//
// Standard spacing scale (all values in rem, Tailwind units)
// 0.5 = 2px, 1 = 4px, 1.5 = 6px, 2 = 8px, 3 = 12px, 4 = 16px, 6 = 24px, 8 = 32px

export const Spacing = {
  // Base unit
  base: 1, // 4px

  // Common spacing values (Tailwind units)
  xs: 0.5,   // 2px  - gap-0.5, p-0.5
  sm: 1,     // 4px  - gap-1, p-1, m-1
  md: 1.5,   // 6px  - gap-1.5, p-1.5, space-y-1.5
  lg: 2,     // 8px  - gap-2, p-2, m-2
  xl: 3,     // 12px - gap-3, p-3, m-3
  '2xl': 4,  // 16px - gap-4, p-4, m-4
  '3xl': 6,  // 24px - gap-6, p-6, m-6
  '4xl': 8,  // 32px - gap-8, p-8, m-8

  // Semantic spacing aliases
  // Form fields
  formGap: 1.5,        // gap-1.5 - gap between label and input
  formFieldGap: 4,     // gap-4 - gap between form fields
  formSectionGap: 6,   // gap-6 - gap between form sections

  // Layout
  cardPadding: 4,      // p-4 for cards
  cardPaddingLg: 6,    // p-6 for large cards
  sectionGap: 6,       // gap-6 between sections
  pagePadding: 4,      // p-4 for page containers
  pagePaddingLg: 6,    // p-6 for large page containers
  headerHeight: 14,    // h-14 for headers (56px)

  // Components
  btnGap: 2,           // gap-2 for button icon+text
  inputGap: 2,         // gap-2 for input icon+text
  iconSize: 4,         // size-4 (16px) for icons
  iconSizeSm: 3,       // size-3 (12px) for small icons
  iconSizeLg: 6,       // size-6 (24px) for large icons

  // Navigation
  navItemGap: 2.5,     // gap-2.5 for nav items
  navPadding: 3,       // p-3 for nav container

  // Grid gaps
  gridGapSm: 2,        // gap-2 for tight grids
  gridGapMd: 4,        // gap-4 for standard grids
  gridGapLg: 6,        // gap-6 for loose grids

  // Container
  containerMaxWidth: 'max-w-7xl', // 80rem = 1280px
} as const

// Tailwind class helpers
export const SpacingClasses = {
  // Gaps
  gapXs: 'gap-0.5',
  gapSm: 'gap-1',
  gapMd: 'gap-1.5',
  gapLg: 'gap-2',
  gapXl: 'gap-3',
  gap2xl: 'gap-4',
  gap3xl: 'gap-6',
  gap4xl: 'gap-8',

  // Padding
  pXs: 'p-0.5',
  pSm: 'p-1',
  pMd: 'p-1.5',
  pLg: 'p-2',
  pXl: 'p-3',
  p2xl: 'p-4',
  p3xl: 'p-6',
  p4xl: 'p-8',

  // Common combinations
  card: 'p-4',
  cardLg: 'p-6',
  section: 'gap-6',
  formField: 'gap-4',
  formGap: 'gap-1.5',
  gridMd: 'gap-4',
  gridLg: 'gap-6',
  page: 'p-4 md:p-6',
} as const

export type SpacingKey = keyof typeof Spacing
export type SpacingClassKey = keyof typeof SpacingClasses
