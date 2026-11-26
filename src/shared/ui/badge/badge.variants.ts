import { cva, type VariantProps } from 'class-variance-authority'

export const badgeVariants = cva(
  'inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-primary text-primary-foreground',
        secondary: 'border-transparent bg-secondary text-secondary-foreground',
        destructive: 'border-transparent bg-destructive-soft text-destructive',
        /** A status without a verdict (draft, archived): the sunken surface, muted text. */
        neutral: 'border-transparent bg-muted text-muted-foreground',
        outline: 'text-foreground',
        /** A figure to read with care (an old rate, a guess): soft warning background. */
        warning: 'border-transparent bg-warning-soft font-medium text-warning',
        /** A settled state (paid, returned): soft positive background. */
        success: 'border-transparent bg-success-soft font-medium text-success',
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
