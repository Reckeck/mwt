/**
 * Browser-mock half of node workflows for `pnpm dev`. The catalog is the real
 * one (written by `mise run gen-types`); a run only walks the flow wires and
 * says which nodes it passed — the executor itself lives in Rust.
 */

export * from './commands'
