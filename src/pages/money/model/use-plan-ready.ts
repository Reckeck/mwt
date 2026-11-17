import { type InjectionKey, inject, onMounted, provide, type Ref, ref, watch } from 'vue'

type Hold = (pending: () => boolean) => void

const PLAN_HOLD: InjectionKey<Hold> = Symbol('plan-hold')

/**
 * Whether the plan has its first data in every block. A block's height follows its data (how many
 * limits, goals and coming items there are), so no skeleton can match it: the plan is shown whole
 * once all of them have it, and nothing below moves when one fills in later (rule 17).
 * Once ready it stays so: a refetch keeps the previous data on screen.
 */
export function providePlanReady(): Readonly<Ref<boolean>> {
  const holds: Array<() => boolean> = []
  const ready = ref(false)
  provide(PLAN_HOLD, (pending) => holds.push(pending))
  // The blocks register in their own setup, which runs before the plan is mounted.
  onMounted(() => {
    watch(
      () => holds.every((pending) => !pending()),
      (settled) => {
        if (settled) ready.value = true
      },
      { immediate: true },
    )
  })
  return ready
}

/** Hold the plan back while this block's first data is on its way; outside the plan it does nothing. */
export function holdPlan(pending: () => boolean): void {
  inject(PLAN_HOLD, null)?.(pending)
}
