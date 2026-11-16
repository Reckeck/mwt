import { CRYPTO_TOURS } from './tours.crypto.ts'
import { TOGETHER_TOURS } from './tours.together.ts'
import type { TourStep } from './tours.types.ts'

/** The tours that live in files of their own, one file per module. */
export const MODULE_TOURS: Record<string, TourStep[]> = { ...TOGETHER_TOURS, ...CRYPTO_TOURS }
