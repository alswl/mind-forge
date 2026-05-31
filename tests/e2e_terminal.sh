#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

MF="${MF_BIN:-/app/mf}"

assert_contains() {
    local label="$1" output="$2" expected="$3"
    if [[ "$output" == *"$expected"* ]]; then
        echo -e "  ${GREEN}PASS${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $label — expected '$expected' not found"
        echo "       output: $output"
        FAIL=$((FAIL + 1))
    fi
}

assert_not_contains() {
    local label="$1" output="$2" unexpected="$3"
    if [[ "$output" != *"$unexpected"* ]]; then
        echo -e "  ${GREEN}PASS${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $label — unexpected '$unexpected' found"
        echo "       output: $output"
        FAIL=$((FAIL + 1))
    fi
}

assert_eq() {
    local label="$1" expected="$2" actual="$3"
    if [[ "$expected" == "$actual" ]]; then
        echo -e "  ${GREEN}PASS${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $label — expected '$expected', got '$actual'"
        FAIL=$((FAIL + 1))
    fi
}

run_mf() {
    local out
    out=$("$MF" "$@" 2>&1) || true
    echo "$out"
}

echo ""
echo "=== $(date '+%H:%M:%S') Terminal Capabilities E2E ==="
echo ""

# ── US1: Modern terminal detection ──────────────────────
echo -e "${CYAN}[US1] Modern terminal recognition${NC}"

out=$(MF_FORCE_TTY=1 TERM=xterm-ghostty COLORTERM=truecolor TERM_PROGRAM=Ghostty "$MF" config terminal 2>&1)
assert_contains "ghostty text: Color= truecolor" "$out" "Color: truecolor"
assert_contains "ghostty text: Hyperlinks= yes" "$out" "Hyperlinks: yes"
assert_contains "ghostty text: TTY= yes" "$out" "TTY: yes"
assert_contains "ghostty text: Terminal ID" "$out" "Terminal: xterm-ghostty"

out=$(MF_FORCE_TTY=1 TERM=xterm-kitty TERM_PROGRAM=kitty "$MF" config terminal 2>&1)
assert_contains "kitty text: Color= truecolor" "$out" "Color: truecolor"

out=$(MF_FORCE_TTY=1 TERM=xterm-ghostty COLORTERM=truecolor TERM_PROGRAM=Ghostty "$MF" --json config terminal 2>&1)
assert_contains "ghostty JSON: status=ok" "$out" '"status": "ok"'
assert_contains "ghostty JSON: color_mode=truecolor" "$out" '"color_mode": "truecolor"'
assert_contains "ghostty JSON: truecolor=true" "$out" '"truecolor": true'
assert_contains "ghostty JSON: hyperlinks=true" "$out" '"hyperlinks": true'
assert_not_contains "ghostty JSON: no ANSI escapes" "$out" $'\x1b['
assert_not_contains "ghostty JSON: no OSC 8" "$out" $'\x1b]8;'

# ── US2: Fallback ──────────────────────────────────────
echo ""
echo -e "${CYAN}[US2] Fallback & safety${NC}"

out=$(MF_FORCE_TTY=1 TERM=xterm-256color "$MF" config terminal 2>&1)
assert_not_contains "xterm-256color: not truecolor" "$out" "Color: truecolor"
assert_contains "xterm-256color: no hyperlinks" "$out" "Hyperlinks: no"

out=$(MF_FORCE_TTY=1 TERM=dumb "$MF" config terminal 2>&1)
assert_contains "dumb: Color=none" "$out" "Color: none"
assert_contains "dumb: Hyperlinks=no" "$out" "Hyperlinks: no"

out=$(MF_FORCE_TTY=1 NO_COLOR=1 TERM=xterm-ghostty COLORTERM=truecolor "$MF" config terminal 2>&1)
assert_contains "NO_COLOR: Color=none" "$out" "Color: none"
assert_contains "NO_COLOR: Hyperlinks=no" "$out" "Hyperlinks: no"
assert_contains "NO_COLOR: fallback reason" "$out" "Fallback: NO_COLOR active"

# Pipe mode (no TTY)
out=$(TERM=xterm-ghostty COLORTERM=truecolor "$MF" config terminal 2>&1)
assert_contains "pipe: TTY=no" "$out" "TTY: no"
assert_contains "pipe: Color=none" "$out" "Color: none"
assert_not_contains "pipe: no ANSI" "$out" $'\x1b['
assert_not_contains "pipe: no OSC 8" "$out" $'\x1b]8;'

# ── US2: Detection precedence ───────────────────────────
echo ""
echo -e "${CYAN}[US2] Detection precedence${NC}"

out=$(MF_FORCE_TTY=1 TERM=xterm-256color COLORTERM=truecolor "$MF" config terminal 2>&1)
assert_contains "precedence: COLORTERM over TERM" "$out" "Color: truecolor"

out=$(MF_FORCE_TTY=1 NO_COLOR=1 TERM=xterm-ghostty COLORTERM=truecolor TERM_PROGRAM=Ghostty "$MF" config terminal 2>&1)
assert_contains "precedence: NO_COLOR wins" "$out" "Color: none"

# ── US3: Diagnostic command ─────────────────────────────
echo ""
echo -e "${CYAN}[US3] Diagnostic command${NC}"

out=$("$MF" config terminal 2>&1)
assert_contains "outside repo: text works" "$out" "Terminal:"
assert_eq "outside repo: exit 0" 0 "$(MF_FORCE_TTY=1 "$MF" config terminal >/dev/null 2>&1; echo $?)"

out=$("$MF" --json config terminal 2>&1)
assert_contains "outside repo: JSON works" "$out" '"status": "ok"'
assert_contains "outside repo: has profile" "$out" '"profile"'
assert_contains "outside repo: has policy" "$out" '"policy"'
assert_contains "outside repo: has environment" "$out" '"environment"'
assert_contains "outside repo: has checks" "$out" '"checks"'
assert_contains "outside repo: has recommendations" "$out" '"recommendations"'

# ── JSON safety ─────────────────────────────────────────
echo ""
echo -e "${CYAN}[US3] JSON safety${NC}"

out=$(MF_FORCE_TTY=1 TERM=xterm-ghostty COLORTERM=truecolor TERM_PROGRAM=Ghostty "$MF" --json config terminal 2>&1)
assert_not_contains "JSON rich: no ANSI" "$out" $'\x1b['
assert_not_contains "JSON rich: no OSC 8" "$out" $'\x1b]8;'

out=$(MF_FORCE_TTY=1 NO_COLOR=1 "$MF" --json config terminal 2>&1)
assert_contains "JSON no-color: status=ok" "$out" '"status": "ok"'
assert_contains "JSON no-color: color_mode=none" "$out" '"color_mode": "none"'
assert_not_contains "JSON no-color: no ANSI" "$out" $'\x1b['

# ── Determinism ─────────────────────────────────────────
echo ""
echo -e "${CYAN}[US3] Determinism${NC}"

out1=$(MF_FORCE_TTY=1 TERM=xterm-256color "$MF" --json config terminal 2>&1)
out2=$(MF_FORCE_TTY=1 TERM=xterm-256color "$MF" --json config terminal 2>&1)
assert_eq "deterministic output" "$out1" "$out2"

# ── Terminfo detection (if infocmp available) ────────────
echo ""
echo -e "${CYAN}[Terminfo] infocmp-based detection${NC}"

out=$(MF_FORCE_TTY=1 TERM=xterm-direct "$MF" config terminal 2>&1)
if command -v infocmp &>/dev/null; then
    assert_contains "xterm-direct: truecolor via terminfo" "$out" "Color: truecolor"
else
    echo "  SKIP  infocmp not available in this environment"
fi

# ── Report ──────────────────────────────────────────────
echo ""
echo "======================================"
TOTAL=$((PASS + FAIL))
if [[ $FAIL -eq 0 ]]; then
    echo -e "  ${GREEN}ALL $PASS tests passed${NC}"
else
    echo -e "  ${GREEN}$PASS passed${NC}  ${RED}$FAIL failed${NC}  ($TOTAL total)"
fi
echo "======================================"

exit $FAIL
