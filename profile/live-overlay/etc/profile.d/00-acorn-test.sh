#!/bin/ash
# AcornOS test mode instrumentation
# Activates ONLY on serial console (ttyS0) - test harness environment
# Users on tty1 see normal behavior

# Only run in interactive shells
case "$-" in
    *i*) ;;
    *) return ;;
esac

# Detect test mode: serial console = test mode
if [ "$(tty)" = "/dev/ttyS0" ]; then
    export ACORN_TEST_MODE=1
else
    # Not test mode - exit early
    return
fi

# ═══════════════════════════════════════════════════════════════════
# TEST MODE ACTIVE - Emit structured markers for install-tests harness
# ═══════════════════════════════════════════════════════════════════

# Signal shell is ready - test harness waits for this
echo "___SHELL_READY___"

# Simple PS1 that emits prompt marker
# ash doesn't have PROMPT_COMMAND, so we use PS1 with command substitution
_acorn_prompt() {
    echo "___PROMPT___"
}

PS1='$(_acorn_prompt)# '
