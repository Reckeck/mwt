import { i18n } from '@/shared/config/i18n'

import { parseDecimal } from '../decimal'

/**
 * Field checks for `useFormValidation`. Each returns the message to show under
 * the field, or `null` when the value is fine. They are plain functions so a
 * form can combine them: `required(name) ?? minLength(name, 3)`.
 */
export type FieldError = string | null

const message = (key: string, params: Record<string, unknown> = {}) => i18n.global.t(`validation.${key}`, params)

const isBlank = (value: unknown) =>
  value === null ||
  value === undefined ||
  (typeof value === 'string' && value.trim() === '') ||
  (Array.isArray(value) && value.length === 0)

/** Something has to be there: a non-blank string, a picked option, a non-empty list. */
export function required(value: unknown, key = 'required'): FieldError {
  return isBlank(value) ? message(key) : null
}

/** A decimal number (comma or dot); blank passes — pair with `required` when it must be set. */
export function decimal(value: string | number | null | undefined): FieldError {
  if (isBlank(value)) return null
  return parseDecimal(String(value)) === null ? message('number') : null
}

/** Blank → «required», not a number → «number», ≤ 0 → «positive». */
export function positiveAmount(value: string | number | null | undefined): FieldError {
  if (isBlank(value)) return message('required')
  const parsed = parseDecimal(String(value))
  if (parsed === null) return message('number')
  return Number(parsed) > 0 ? null : message('positive')
}

/** Optional, but when given: a number not below zero. */
export function nonNegativeAmount(value: string | number | null | undefined): FieldError {
  if (isBlank(value)) return null
  const parsed = parseDecimal(String(value))
  if (parsed === null) return message('number')
  return Number(parsed) >= 0 ? null : message('nonNegative')
}

/** A whole number within bounds; blank passes. */
export function integerInRange(value: string | number | null | undefined, min: number, max: number): FieldError {
  if (isBlank(value)) return null
  const parsed = Number(String(value).trim())
  if (!Number.isInteger(parsed)) return message('integer')
  return parsed < min || parsed > max ? message('range', { min, max }) : null
}

export function minLength(value: string, min: number): FieldError {
  return value.length < min ? message('minLength', { min }) : null
}

/** The first error of several checks, for a field with more than one rule. */
export function firstError(...errors: FieldError[]): FieldError {
  return errors.find((error) => error) ?? null
}
