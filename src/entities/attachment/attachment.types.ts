export interface LinkDraft {
  url: string
  title: string
}

/** A file or link picked in a form before its owner exists; attached right after the save. */
export type StagedAttachment =
  | { key: number; kind: 'file'; name: string; file: File }
  | { key: number; kind: 'link'; url: string; title: string }
