export interface DeltaProps {
  /** The change as a number (`4.2`, `-1`) or a decimal string; its sign picks the colour and the sign mark. */
  value: number | string
  /** The text after the sign, already formatted and rounded (`4.2%`) and unsigned: the sign mark is added here, so a signed text doubles it. Defaults to the absolute value. */
  text?: string
  /** For a figure where a rise is bad (expenses): flips which direction is green. */
  inverse?: boolean
}
