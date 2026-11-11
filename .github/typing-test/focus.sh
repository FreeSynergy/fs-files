# Scenario of the typing test (fs-release actions/typing-test): defines focus(), which brings a text field of the window
# into focus. The window opens on the list of files; "Find" opens the search form, and its first field ("Name") takes the
# text. Positions are relative to the window's top left corner (WX0, WY0, set by the test; title bar included), for the
# window size of the workflow (`--size 1000x640`).
focus() {
  c "move $((WX0 + 278)) $((WY0 + 175))" 0.4
  c "click $((WX0 + 278)) $((WY0 + 175))" 1.5
  local x=$((WX0 + 690)) y=$((WY0 + 261))
  TT_NEAR="$x,$y"; export TT_NEAR
  c "move $x $y" 0.4
  c "click $x $y" 1
}
