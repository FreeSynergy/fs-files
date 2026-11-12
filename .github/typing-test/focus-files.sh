# Scenario of the typing test (fs-release actions/typing-test) for the standard window of fs-files, the file window:
# defines focus(), which brings the search field at the right of the bar of the way into focus. Positions are relative
# to the window's top left corner (WX0, WY0, set by the test; title bar included), for the window size of the workflow
# (`--size 1000x640`): the search field is 220 px wide at the right edge, its middle line is 144 px below the corner.
focus() {
  local x=$((WX0 + 890)) y=$((WY0 + 144))
  TT_NEAR="$x,$y"; export TT_NEAR
  c "move $x $y" 0.4
  c "click $x $y" 1
}
