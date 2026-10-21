import { integerInRange } from '@/shared/lib/form'

/** A car's fields as the form edits them: text in every input. */
export interface CarDraft {
  model: string
  year: string
  plate: string
  osagoUntil: string
  kaskoUntil: string
  serviceDueAt: string
  serviceDueKm: string
  serviceIntervalMonths: string
  serviceIntervalKm: string
}

const text = (value: number | null | undefined) => (value === null || value === undefined ? '' : String(value))
const whole = (value: string) => (value.trim() ? Number(value.trim()) : null)

export function carDraftOf(car: CarInfo | undefined): CarDraft {
  return {
    model: car?.model ?? '',
    year: text(car?.year),
    plate: car?.plate ?? '',
    osagoUntil: car?.osago_until ?? '',
    kaskoUntil: car?.kasko_until ?? '',
    serviceDueAt: car?.service_due_at ?? '',
    serviceDueKm: text(car?.service_due_km),
    serviceIntervalMonths: text(car?.service_interval_months),
    serviceIntervalKm: text(car?.service_interval_km),
  }
}

/** The saved car with the draft on top; the mileage is changed by its own action. */
export function carInfoOf(draft: CarDraft, saved: CarInfo | undefined): CarInfo {
  return {
    model: draft.model.trim(),
    year: whole(draft.year),
    plate: draft.plate.trim(),
    osago_until: draft.osagoUntil || null,
    kasko_until: draft.kaskoUntil || null,
    service_due_at: draft.serviceDueAt || null,
    service_due_km: whole(draft.serviceDueKm),
    service_interval_months: whole(draft.serviceIntervalMonths),
    service_interval_km: whole(draft.serviceIntervalKm),
    mileage_km: saved?.mileage_km ?? null,
    mileage_at: saved?.mileage_at ?? null,
  }
}

const KM_MAX = 5_000_000

/** Field errors of a car draft, keyed like the inputs. */
export function carErrors(draft: CarDraft) {
  return {
    carYear: integerInRange(draft.year, 1900, 2100),
    carServiceKm: integerInRange(draft.serviceDueKm, 0, KM_MAX),
    carIntervalMonths: integerInRange(draft.serviceIntervalMonths, 1, 120),
    carIntervalKm: integerInRange(draft.serviceIntervalKm, 1, KM_MAX),
  }
}

export type CarField = keyof ReturnType<typeof carErrors>
