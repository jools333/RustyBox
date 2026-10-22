#!/bin/sh
set -e

case "$1" in
    purge|remove)
        rm -f /etc/polkit-1/rules.d/60-rustybox-resolved.rules
        rm -f /usr/share/polkit-1/rules.d/60-rustybox-resolved.rules
        ;;
    *)
        ;;
esac

exit 0
