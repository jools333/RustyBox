#!/bin/sh
set -e

# Automatically configure Linux network capabilities for RustyBox's sing-box core.
# This allows sing-box to configure TUN virtual adapters and routing tables
# without requiring the GUI or the user to run as root or enter sudo passwords.

case "$1" in
    configure)
        CANDIDATES="
            /usr/lib/rustybox/bin/sing-box
            /usr/lib/RustyBox/bin/sing-box
            /usr/lib/rustybox/sing-box
            /usr/lib/com.rustybox.app/bin/sing-box
        "
        for bin in $CANDIDATES; do
            if [ -f "$bin" ]; then
                chmod 755 "$bin"
                if command -v setcap >/dev/null 2>&1; then
                    setcap cap_net_admin,cap_net_bind_service=+ep "$bin" 2>/dev/null || true
                fi
            fi
        done
        ;;
    abort-upgrade|abort-remove|abort-deconfigure)
        ;;
    *)
        ;;
esac

exit 0
