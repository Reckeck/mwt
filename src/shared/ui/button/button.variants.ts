import { cva, type VariantProps } from 'class-variance-authority'

export const buttonVariants = cva(
  "touch-hit inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground shadow hover:bg-primary/90',
        destructive: 'bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90',
        outline: 'border border-border bg-background shadow-sm hover:bg-accent hover:text-accent-foreground',
        secondary: 'bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80',
        ghost: 'hover:bg-accent hover:text-accent-foreground',
        link: 'text-primary underline-offset-4 hover:underline',
      },
      /** On a touch screen the text sizes and `icon` are at least 40px tall (rule 16; Button · Size 40px). */
      size: {
        default: 'h-9 px-4 py-2 touch-min',
        sm: 'h-8 rounded-md px-3 text-xs touch-min',
        lg: 'h-10 rounded-md px-8',
        icon: 'size-9 touch-min-square',
        'icon-sm': 'size-8 touch-hit-lg',
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
