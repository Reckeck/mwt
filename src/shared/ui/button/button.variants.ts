import { cva, type VariantProps } from 'class-variance-authority'

export const buttonVariants = cva(
  "touch-hit inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors select-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      /** Hover and pressed mix the foreground into the fill (`color-mix`), so no state needs its own colour. */
      variant: {
        default: [
          'bg-primary text-primary-foreground',
          'hover:bg-[color-mix(in_oklab,var(--primary),var(--primary-foreground)_14%)]',
          'active:bg-[color-mix(in_oklab,var(--primary),var(--primary-foreground)_24%)]',
        ],
        destructive: [
          'bg-destructive text-destructive-foreground',
          'hover:bg-[color-mix(in_oklab,var(--destructive),var(--destructive-foreground)_12%)]',
          'active:bg-[color-mix(in_oklab,var(--destructive),var(--destructive-foreground)_22%)]',
        ],
        outline: [
          'border border-border bg-background',
          'hover:bg-[color-mix(in_oklab,var(--background),var(--foreground)_6%)]',
          'active:bg-[color-mix(in_oklab,var(--background),var(--foreground)_12%)]',
        ],
        secondary: [
          'bg-secondary text-secondary-foreground',
          'hover:bg-[color-mix(in_oklab,var(--secondary),var(--foreground)_8%)]',
          'active:bg-[color-mix(in_oklab,var(--secondary),var(--foreground)_14%)]',
        ],
        ghost: [
          'bg-transparent',
          'hover:bg-[color-mix(in_oklab,transparent,var(--foreground)_8%)]',
          'active:bg-[color-mix(in_oklab,transparent,var(--foreground)_14%)]',
        ],
        link: 'text-primary underline-offset-4 hover:underline',
        /** A select / filter / removable chip: a pill that turns ink when picked (`aria-pressed`, `data-state=on`). */
        chip: [
          'rounded-full border border-border bg-background text-foreground',
          'hover:bg-[color-mix(in_oklab,var(--background),var(--foreground)_6%)]',
          'active:bg-[color-mix(in_oklab,var(--background),var(--foreground)_12%)]',
          'aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground',
          'data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground',
        ],
      },
      /** On a touch screen the text sizes and `icon` are at least 40px tall (rule 16; Button · Size 40px). */
      size: {
        default: 'h-9 px-4 py-2 touch-min',
        sm: 'h-8 rounded-md px-3 text-xs touch-min',
        lg: 'h-10 rounded-md px-8',
        icon: 'size-9 touch-min-square',
        'icon-sm': 'size-8 touch-hit-lg',
        /** A chip: compact pill with room for a count and a «×». */
        chip: 'h-8 gap-1.5 px-3 text-xs touch-min',
        /** A small «×» or icon inside a chip or a list row. */
        'icon-xs': 'size-6 rounded-sm touch-hit-lg',
        /** A list row or a card that is one big button: the content lays itself out. */
        block:
          'h-auto justify-start whitespace-normal rounded-md px-2 py-1.5 text-left font-normal pointer-coarse:min-h-10',
      },
    },
    /** A link inside a sentence keeps the line height: its `touch-hit` area is the target. */
    compoundVariants: [{ variant: 'link', class: 'pointer-coarse:min-h-0' }],
    defaultVariants: { variant: 'default', size: 'default' },
  },
)

export type ButtonVariants = VariantProps<typeof buttonVariants>

export type ButtonVariant = ButtonVariants['variant']

export type ButtonSize = ButtonVariants['size']

export interface ButtonProps {
  variant?: ButtonVariant
  size?: ButtonSize
  /**
   * Waiting for the action: the label stays in place (invisible) under a
   * spinner, so the button keeps its width (rule 17); it is disabled meanwhile.
   */
  loading?: boolean
}
