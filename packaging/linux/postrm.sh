#!/bin/sh
# After the deb/rpm package is removed: take the app out of every user's
# autostart. The entry lives in the home directory (tauri-plugin-autostart),
# so no package manager removes it, and a session would keep launching a
# program that is gone. Only entries that start the packaged binary go.
#
# deb passes remove|purge|upgrade|..., rpm passes the count of installs left
# (0 = erase, 1 = upgrade), pacman's post_remove nothing.
case "${1:-remove}" in
  remove | purge | 0) ;;
  *) exit 0 ;;
esac
for entry in /home/*/.config/autostart/*.desktop /root/.config/autostart/*.desktop; do
  [ -f "$entry" ] || continue
  if grep -q '^Exec=/usr/bin/multi-wallet-tracker\( \|$\)' "$entry"; then
    rm -f "$entry"
  fi
done
exit 0
