# laye holds only exemplary commits — each one a complete, working thing.
# The tag before the colon says which track a commit belongs to and where in
# that track it sits.
#
#   <TRACK><n>[<step>]   S1  S6a  R7a  R16  M2g  B3a  O4  N1  P1
#   <ALLCAPS>            FIX  DEP  LAYE  WONTUSE  INFRAFIX
#
# The lowercase step letter is deliberate, not drift: B3a -> B3b -> B3c is a
# run of complete things inside one milestone, and S6/S6a, R7/R7a do the same.
#
# Anything outside a track collapses into an all-caps word. That is the escape
# hatch, and it is the only one — lowercase tags (refactor:, ci:, relaye:) and
# untagged subjects are exactly what this exists to stop.
#
# Sequences are tried in declaration order and the first to consume a valid
# prefix wins, so the most specific comes first: S6a must not be read as S6.
scope {
  path: "/teranos/laye"
  event: "PreToolUse"

  control {
    name: "laye-commit-format"
    cmd: "git commit"
    strop {
      flag: "-m"

      # M2f (bevy-starter):  — parenthesised slice, hyphenated qualifier
      sequence [ letters(1..1) digits(1..2) lower(1..1) literal(" (") lower(1..12) literal("-") lower(1..12) literal("):") ]
      # M2i (wire):          — parenthesised slice
      sequence [ letters(1..1) digits(1..2) lower(1..1) literal(" (") lower(1..12) literal("):") ]
      # M2i.c:               — dotted sub-sub-step
      sequence [ letters(1..1) digits(1..2) lower(1..1) literal(".") lower(1..1) literal(":") ]
      # M2ka:                — same shape, no separator
      sequence [ letters(1..1) digits(1..2) lower(1..1) lower(1..1) literal(":") ]
      # S6a:                 — sub-step
      sequence [ letters(1..1) digits(1..2) lower(1..1) literal(":") ]
      # S6:                  — track + number
      sequence [ letters(1..1) digits(1..2) literal(":") ]
      # LS-LK:               — hyphenated Phase LAYE pair
      sequence [ letters(2..2) literal("-") letters(2..2) literal(":") ]
      # FIX:  LS:            — all-caps word, and the Phase LAYE codes
      sequence [ letters(2..10) literal(":") ]
    }
  }
}
