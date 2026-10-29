# Keyboard, focus, accessibility, animation

## Keyboard

- Everything done with the mouse can be done from the keyboard: Tab reaches
  every element, Enter/Space activates it.
- Enter in a form saves (`<form>` or `FormDialog`); in a multiline field it is
  Ctrl+Enter. Esc closes a dialog, popover, menu (Reka does this).
- Something clickable is a `Button`, a link is a `RouterLink`. `div @click` is
  allowed only with `role`, `tabindex="0"` and Enter handling (this is how
  `DataTable` rows with `clickable` are built).
- Hotkeys are shown in a hint (`Kbd`) and must not break on a non-Latin
  (e.g. Russian) keyboard layout: check `event.code`, not `event.key`.

## Focus

- Focus is visible: `focus-visible:ring-2 focus-visible:ring-ring` (the
  `shared/ui` components already have it; your own button made from `<button>`
  is a `check-ui` violation).
- An open dialog puts focus on the first field (in Reka — `@open-auto-focus`);
  a closed one returns it to the button that opened it.
- After a failed save, focus goes to the first field with an error (done by
  `useFormValidation`).

## ARIA and meaning

- An icon button gets `aria-label` (and `title` for the mouse hint).
- A field has a label (`Label for`); the hint/error is linked to the field.
- An error message is `role="alert"` (`FormField`, `SectionState`).
- A sortable table header gets `aria-sort` (done by `DataTable`).
- Status is not conveyed by color alone: "Archive" as a word, P&L with a sign.
- Text contrast is at least 4.5:1; check `text-muted-foreground` on
  `bg-muted` in the dark theme.

## Animation

- Animation explains a transition (a dialog appeared, a group expanded); it
  does not decorate frequent actions (open a menu, add a row).
- Animate `transform` and `opacity`, not `transition-all` (`check-ui`); up to
  ~200 ms; the animation can be interrupted.
- `prefers-reduced-motion: reduce` turns off animations and transitions
  globally (`styles.css`) — your own code does nothing for this, but does not
  bypass it either.
- Theme switching does not animate colors.

Sources: Vercel Web Interface Guidelines, Rauno Freiberg "Interfaces"
(interfaces.rauno.me), WAI-ARIA Authoring Practices.
