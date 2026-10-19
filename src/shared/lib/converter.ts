import { ref } from 'vue'

/** The converter dialog: one for the app, drawn by the shell, opened from anywhere. */
export const converterOpen = ref(false)
/** The currency it starts from (a wallet's own), `null` for the base. */
export const converterFrom = ref<string | null>(null)

export function openConverter(from: string | null = null) {
  converterFrom.value = from
  converterOpen.value = true
}
