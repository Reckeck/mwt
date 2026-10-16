import type { InjectionKey, Ref } from 'vue'

/**
 * reka-ui reserves the empty string for «nothing picked», but the app's
 * selects use `''` for a real choice («Любой», «—», «Без контакта»). Inside
 * the list that choice travels under this value; the model still sees `''`.
 */
export const EMPTY_VALUE = '__select-empty__'

/** How many `SelectItem value=""` the select holds: with none, `''` means «nothing picked». */
export const SELECT_EMPTY_ITEMS: InjectionKey<Ref<number>> = Symbol('select-empty-items')
