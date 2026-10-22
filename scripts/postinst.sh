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

        # Automatically configure Polkit rule for systemd-resolved.
        # When sing-box uses TUN in auto_route mode, it configures per-link DNS via D-Bus / resolvectl.
        # This rule allows active local desktop users to apply TUN DNS settings without password prompts.
        POLKIT_RULES_DIR="/etc/polkit-1/rules.d"
        if [ ! -d "$POLKIT_RULES_DIR" ]; then
            POLKIT_RULES_DIR="/usr/share/polkit-1/rules.d"
        fi

        if [ -d "$POLKIT_RULES_DIR" ]; then
            cat << 'EOF' > "$POLKIT_RULES_DIR/60-rustybox-resolved.rules"
// Allow active local desktop users to configure per-link DNS via systemd-resolved
// for RustyBox VPN without interactive Polkit password prompts.
polkit.addRule(function(action, subject) {
    if ((action.id == "org.freedesktop.resolve1.set-dns-servers" ||
         action.id == "org.freedesktop.resolve1.set-domains" ||
         action.id == "org.freedesktop.resolve1.set-default-route" ||
         action.id == "org.freedesktop.resolve1.revert") &&
        subject.active == true && subject.local == true) {
        return polkit.Result.YES;
    }
});
EOF
            chmod 644 "$POLKIT_RULES_DIR/60-rustybox-resolved.rules"
        fi
        ;;
    abort-upgrade|abort-remove|abort-deconfigure)
        ;;
    *)
        ;;
esac

exit 0
