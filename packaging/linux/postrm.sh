#!/bin/sh
# After the deb/rpm package is removed: take the app out of every user's
# autostart. The entry lives in the home directory (tauri-plugin-autostart),
# so no package manager removes it, and a session would keep launching a
# program that is gone. Only entries that start a packaged binary that is
# now gone go: the release (multi-wallet-tracker) and the dev app
# (multi-wallet-tracker-dev) install side by side.
#
# deb passes remove|purge|upgrade|..., rpm passes the count of installs left
# (0 = erase, 1 = upgrade), pacman's post_remove nothing.
case "${1:-remove}" in
  remove | purge | 0) ;;
  *) exit 0 ;;
esac
for entry in /home/*/.config/autostart/*.desktop /root/.config/autostart/*.desktop; do
  [ -f "$entry" ] || continue
  program="$(sed -n 's/^Exec=\([^ ]*\).*/\1/p' "$entry" | head -n 1)"
  case "$program" in
    /usr/bin/multi-wallet-tracker | /usr/bin/multi-wallet-tracker-dev)
      [ -e "$program" ] || rm -f "$entry" ;;
  esac
done
exit 0
