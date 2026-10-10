export const UTILITY_KINDS = ['metered', 'static', 'housing'] as const
export type UtilityKindName = (typeof UTILITY_KINDS)[number]
