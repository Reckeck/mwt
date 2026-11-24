/** One pill look for `Button` and `Toggle` `chip`: border, ink when picked (`aria-pressed`, `data-state=on`). */
export const CHIP_CLASSES = [
  'rounded-full border border-border bg-background text-foreground',
  'hover:bg-[color-mix(in_oklab,var(--background),var(--foreground)_6%)]',
  'active:bg-[color-mix(in_oklab,var(--background),var(--foreground)_12%)]',
  'aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground',
  'data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground',
]
