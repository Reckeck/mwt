import { ref } from 'vue'

/**
 * Whether this device holds exchange keys; with keys the autolock cannot be
 * turned off. Stub until H2 keeps the keys: the answer then comes from
 * `security_state().has_device_keys` and this is the only place to change.
 */
export function useHasDeviceKeys() {
  return ref(false)
}
