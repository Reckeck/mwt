import { type QueryClient, useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** A car changed: the wallet, the tree and the reminders built from its papers read it. */
function invalidateVehicle(queryClient: QueryClient) {
  for (const queryKey of [['stores'], ['store'], ['reminders'], ['upcoming']]) {
    void queryClient.invalidateQueries({ queryKey })
  }
}

export function useMileageUpdate() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: VehicleMileageInput) => call<Store>('vehicle_update_mileage', { input }),
    onSuccess: () => invalidateVehicle(queryClient),
    meta: { error: 'car.failed', success: 'car.mileageSaved' },
  })
}

export function useServiceDone() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: VehicleServiceDoneInput) => call<Store>('vehicle_service_done', { input }),
    onSuccess: () => invalidateVehicle(queryClient),
    meta: { error: 'car.failed', success: 'car.serviceSaved' },
  })
}
