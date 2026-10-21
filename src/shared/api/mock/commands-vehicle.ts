/** Commands: a car's mileage and service, as `service/vehicle.rs`. */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockStore } from './tree'

function carOf(storeId: string): CarInfo {
  const store = mockStore(storeId)
  if (store?.property?.category !== 'car') throw { code: 'INVALID_INPUT', message: 'not a car' }
  store.property.car ??= {
    model: '',
    year: null,
    plate: '',
    osago_until: null,
    kasko_until: null,
    service_due_at: null,
    service_due_km: null,
    service_interval_months: null,
    service_interval_km: null,
    mileage_km: null,
    mileage_at: null,
  }
  return store.property.car
}

const today = () => new Date().toISOString().slice(0, 10)

export const vehicleCommands: MockCommands = {
  vehicle_update_mileage: ({ body }) => {
    const input = body as VehicleMileageInput
    const car = carOf(input.store_id)
    car.mileage_km = input.km
    car.mileage_at = today()
    return resolve(mockStore(input.store_id))
  },
  vehicle_service_done: ({ body }) => {
    const input = body as VehicleServiceDoneInput
    const car = carOf(input.store_id)
    if (input.km !== null) {
      car.mileage_km = input.km
      car.mileage_at = input.date
    }
    const date = new Date(input.date)
    if (car.service_interval_months) {
      date.setMonth(date.getMonth() + car.service_interval_months)
      car.service_due_at = date.toISOString().slice(0, 10)
    } else car.service_due_at = null
    car.service_due_km =
      car.service_interval_km && car.mileage_km !== null ? car.mileage_km + car.service_interval_km : null
    return resolve(mockStore(input.store_id))
  },
}
