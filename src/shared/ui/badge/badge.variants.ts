import { cva, type VariantProps } from 'class-variance-authority'

export const badgeVariants = cva(
  'inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-primary text-primary-foreground shadow',
        secondary: 'border-transparent bg-secondary text-secondary-foreground',
        destructive: 'border-transparent bg-destructive text-destructive-foreground shadow',
        outline: 'text-foreground',
        /** A figure to read with care (an old rate, a guess): the warning colour, its own border. */
        warning: 'border-current font-medium text-warning',
        /** A settled state (paid, returned): the success colour, its own border. */
        success: 'border-current font-medium text-success',
      },
    },
    defaultVariants: { variant: 'default' },
  },
)

export type BadgeVariants = VariantProps<typeof badgeVariants>

export type BadgeVariant = BadgeVariants['variant']

export interface BadgeProps {
  variant?: BadgeVariant
}
