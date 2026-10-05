/// Reader for the JSON Schema that `trade-core` derives from `VaultDoc`.
///
/// The document shape lives in exactly one place — the Rust structs — and the
/// editor asks for it over IPC (`vault_schema`). This file only interprets
/// what comes back, so nothing about the vault is duplicated on the frontend.
///
/// What has to be handled, because `schemars` emits all of it:
/// - `$ref` into `$defs`, with the target possibly itself a `oneOf`;
/// - `anyOf` wrapping a value together with `{"type": "null"}` for `Option<T>`;
/// - `type` given as an array (`["string", "null"]`);
/// - enums split between a flat `enum` and `oneOf` of `const`, depending on
///   whether the Rust variants carried doc comments.

export type JsonSchema = Record<string, unknown>

function isRecord(value: unknown): value is JsonSchema {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** `anyOf` or `oneOf` as an array, whichever is present. */
function branchesOf(node: JsonSchema): unknown[] {
  const branches = node.anyOf ?? node.oneOf
  return Array.isArray(branches) ? branches : []
}

/** The first line of a schema description — doc comments are often paragraphs. */
export function descriptionOf(node: JsonSchema): string | undefined {
  const description = node.description
  if (typeof description !== 'string') return undefined
  const firstLine = description.split('\n')[0].trim()
  return firstLine || undefined
}

/** Walk a schema the way a value at that position is shaped. */
export class JsonSchemaReader {
  private readonly root: JsonSchema
  private readonly cache = new Map<string, JsonSchema>()

  constructor(root: JsonSchema) {
    this.root = root
  }

  /** The `$ref` target, or the node itself when it is inline. */
  resolve(node: JsonSchema): JsonSchema {
    const ref = node.$ref
    if (typeof ref !== 'string') return node

    const cached = this.cache.get(ref)
    if (cached) return cached

    // Only local pointers are produced, of the form `#/$defs/Name`.
    const path = ref.replace(/^#\//, '').split('/')
    let target: unknown = this.root
    for (const segment of path) {
      target = isRecord(target) ? target[segment] : undefined
    }
    const resolved = isRecord(target) ? target : {}
    this.cache.set(ref, resolved)
    return resolved
  }

  /** Every `anyOf`/`oneOf` branch that is not the `null` type. */
  private nonNullBranches(node: JsonSchema): JsonSchema[] {
    return branchesOf(node).filter((branch) => {
      const record = isRecord(branch) ? branch : {}
      const type = record.type
      return !(type === 'null' || (Array.isArray(type) && type.length === 1 && type[0] === 'null'))
    }) as JsonSchema[]
  }

  /** Whether `null` is an accepted value here, i.e. the Rust field is `Option`. */
  nullable(node: JsonSchema): boolean {
    if (branchesOf(node).some((branch) => isRecord(branch) && branch.type === 'null')) return true
    return Array.isArray(node.type) && node.type.includes('null')
  }

  /**
   * The node describing the value itself.
   *
   * `Option<T>` is a union of `T` and `null`, so it collapses to `T`. Any
   * other union is left intact: an enum whose variants carried doc comments
   * comes back as `oneOf` of `const`/`enum`, and taking the first branch there
   * would silently drop the documented variants.
   */
  concrete(node: JsonSchema): JsonSchema {
    if (node.$ref) return this.concrete(this.resolve(node))
    const branches = branchesOf(node)
    const real = this.nonNullBranches(node)
    if (branches.length > 1 && real.length === 1) return this.concrete(real[0])
    return node
  }

  /** Field names of an object node, resolved. */
  properties(node: JsonSchema): Record<string, JsonSchema> {
    const properties = this.concrete(node).properties
    if (!isRecord(properties)) return {}
    const result: Record<string, JsonSchema> = {}
    for (const [name, schema] of Object.entries(properties)) {
      if (isRecord(schema)) result[name] = schema
    }
    return result
  }

  /** What a single value at this position may be. */
  leafKind(node: JsonSchema): LeafKind {
    const resolved = this.concrete(node)
    if (this.enumValues(resolved).length > 0) return 'enum'
    const type = resolved.type
    if (type === 'array') return 'array'
    if (isRecord(resolved.properties)) return 'object'
    switch (type) {
      case 'string':
        return 'string'
      case 'integer':
      case 'number':
        return 'number'
      case 'boolean':
        return 'boolean'
      case 'object':
        return 'object'
      default:
        return 'unknown'
    }
  }

  /**
   * Allowed values, gathered from every spelling `schemars` uses: a flat
   * `enum`, a `const`, or a `oneOf` mixing both.
   */
  enumValues(node: JsonSchema): string[] {
    const resolved = this.concrete(node)
    const values: string[] = []

    if (Array.isArray(resolved.enum)) {
      for (const value of resolved.enum) if (typeof value === 'string') values.push(value)
    }
    if (typeof resolved.const === 'string') {
      values.push(resolved.const)
    }
    for (const branch of branchesOf(resolved)) {
      if (!isRecord(branch)) continue
      if (branch.$ref) {
        values.push(...this.enumValues(branch))
        continue
      }
      if (typeof branch.const === 'string') values.push(branch.const)
      if (Array.isArray(branch.enum)) {
        for (const value of branch.enum) if (typeof value === 'string') values.push(value)
      }
    }
    return [...new Set(values)]
  }

  /** Short type description, for the completion detail column. */
  describe(node: JsonSchema): string {
    const resolved = this.concrete(node)
    const values = this.enumValues(resolved)
    if (values.length > 0) return values.join(' | ')
    if (resolved.type === 'array') return `${this.describe((resolved.items as JsonSchema) ?? {})}[]`
    return typeof resolved.type === 'string' ? resolved.type : 'any'
  }

  /** The document root schema. */
  get rootSchema(): JsonSchema {
    return this.root
  }
}

type LeafKind = 'string' | 'number' | 'boolean' | 'object' | 'array' | 'enum' | 'unknown'
