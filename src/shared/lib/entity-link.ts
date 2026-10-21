import { computed } from 'vue'
import { type LocationQuery, type RouteLocationRaw, useRoute, useRouter } from 'vue-router'

/**
 * Addresses of entities (plan J1). Every entity opens as a card over the
 * current screen: `?open=goal:g1~tx:t9` is a stack, the last one on top.
 * A wallet, a workflow and a calculator scenario have pages of their own.
 */
export const ENTITY_KINDS: readonly EntityKind[] = [
  'store',
  'tx',
  'category',
  'contact',
  'goal',
  'budget',
  'automation',
  'workflow',
  'reminder',
  'alert',
  'asset',
  'scenario',
]

/** The query key of the card stack. */
export const OPEN_QUERY_KEY = 'open'
const STACK_SEPARATOR = '~'
/** Deeper than this, the oldest card drops off: the address stays short. */
const STACK_LIMIT = 8

/** `kind:id`; an asset id may hold colons itself. */
export function entityKey(target: EntityRef): string {
  return `${target.kind}:${target.id}`
}

export function parseEntityKey(raw: string): EntityRef | null {
  const colon = raw.indexOf(':')
  if (colon <= 0) return null
  const kind = raw.slice(0, colon) as EntityKind
  const id = raw.slice(colon + 1)
  return ENTITY_KINDS.includes(kind) && id ? { kind, id } : null
}

export function sameEntity(left: EntityRef | null | undefined, right: EntityRef | null | undefined): boolean {
  return !!left && !!right && left.kind === right.kind && left.id === right.id
}

/** The open cards from the query, bottom first. */
export function openStack(query: LocationQuery): EntityRef[] {
  const raw = query[OPEN_QUERY_KEY]
  const value = Array.isArray(raw) ? raw[0] : raw
  if (typeof value !== 'string' || !value) return []
  return value
    .split(STACK_SEPARATOR)
    .map(parseEntityKey)
    .filter((target): target is EntityRef => target !== null)
}

/** The query with `stack` as its open cards (none: the key is dropped). */
export function withOpenStack(query: LocationQuery, stack: EntityRef[]): LocationQuery {
  const { [OPEN_QUERY_KEY]: _open, ...rest } = query
  if (!stack.length) return rest
  return { ...rest, [OPEN_QUERY_KEY]: stack.slice(-STACK_LIMIT).map(entityKey).join(STACK_SEPARATOR) }
}

/** The page of an entity that has one: a wallet, a workflow, a calculator scenario. */
export function entityPage(target: EntityRef): RouteLocationRaw | null {
  if (target.kind === 'store') return { name: 'wallet', params: { id: target.id } }
  if (target.kind === 'workflow') return { name: 'workflow-edit', params: { id: target.id } }
  if (target.kind === 'scenario') return { name: 'planning-calculator', query: { scenario: target.id } }
  return null
}

/**
 * Where a link to `target` leads from the current address: its page, or the
 * card pushed onto the stack. A card already on top is not pushed twice.
 */
export function entityLocation(target: EntityRef, query: LocationQuery): RouteLocationRaw {
  const page = entityPage(target)
  if (page) return page
  const stack = openStack(query)
  if (sameEntity(stack.at(-1), target)) return { query }
  return { query: withOpenStack(query, [...stack, target]) }
}

type Opener = (target: EntityRef) => void
let opener: Opener | null = null

/** The shell registers how to open a card, for code outside components. */
export function registerEntityOpener(open: Opener): () => void {
  opener = open
  return () => {
    if (opener === open) opener = null
  }
}

/** Open the card of `target` from anywhere; `false` before the shell is up. */
export function openEntity(target: EntityRef): boolean {
  if (!opener) return false
  opener(target)
  return true
}

/** Open, step back from and close cards on the current route. */
export function useEntityNav() {
  const route = useRoute()
  const router = useRouter()
  const stack = computed(() => openStack(route.query))
  const top = computed(() => stack.value.at(-1) ?? null)

  function open(target: EntityRef) {
    void router.push(entityLocation(target, route.query))
  }
  /** Drop the top card: the one under it shows again. */
  function back() {
    void router.replace({ query: withOpenStack(route.query, stack.value.slice(0, -1)) })
  }
  function closeAll() {
    void router.replace({ query: withOpenStack(route.query, []) })
  }
  return { stack, top, open, back, closeAll }
}
