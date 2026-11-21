Package "kb:braille-display@0.1.0"

DefType command {opcode: u8 x: u16 y: u16 w: u16 h: u16 first: u8 second: u8 third: u8}
DefType display-frame {commands: list(command) labels: list(string)}
DefType display-entry {label: string cells: list(u16) mnemonic: string}

Def display-empty {commands: The list(command) [] labels: The list(string) []}

Def display-box Fn {frame: display-frame opcode: u8 x: s64 y: s64 w: s64 h: s64 color: u8 radius: u8}
  Match frame [({commands: commands labels: labels}
    {commands: push(commands {opcode: opcode x: to-u16(x) y: to-u16(y)
                              w: to-u16(w) h: to-u16(h) first: color second: radius third: 0})
     labels: labels})]

Def display-text Fn {frame: display-frame x: s64 y: s64 color: u8 size: u8 text: string}
  Match frame [({commands: commands labels: labels}
    {commands: push(commands {opcode: 3 x: to-u16(x) y: to-u16(y)
                              w: 0 h: 0 first: color second: size third: to-u8(len(labels))})
     labels: push(labels text)})]

Def display-dot-bit Fn {cell: s64 from: s64 to: s64}
  mul(rem(div(cell from) 2) to)

Def display-dot-mask Fn {cell: s64}
  Let {row-one: add(display-dot-bit(cell 1 1) display-dot-bit(cell 8 2))
       row-two: add(display-dot-bit(cell 2 4) display-dot-bit(cell 16 8))
       row-three: add(display-dot-bit(cell 4 16) display-dot-bit(cell 32 32))
       row-four: mul(rem(div(cell 64) 4) 64)}
    to-u8(add(add(row-one row-two) add(row-three row-four)))

Def display-dot-grid Fn {frame: display-frame x: s64 y: s64 w: s64 h: s64 cell: u16 extra-gap: bool}
  Let {bits: to-s64(cell)
       style: If eq(rem(div(bits 256) 2) 1) 132 3}
    Match frame [({commands: commands labels: labels}
      {commands: push(commands {opcode: 4 x: to-u16(x) y: to-u16(y)
                                w: to-u16(w) h: to-u16(h)
                                first: display-dot-mask(bits)
                                second: to-u8(add(style If extra-gap 64 0)) third: 66})
       labels: labels})]

Def display-dots Fn {frame: display-frame x: s64 y: s64 w: s64 h: s64 cell: u16}
  display-dot-grid(frame x y w h cell false)

Def display-get Fn {model: list(u8) index: s64 fallback: u8}
  If lt(index len(model)) get(model index) fallback

Def display-get-wide Fn {model: list(u8) index: s64}
  to-u16(add(to-s64(display-get(model index 0)) mul(256 to-s64(display-get(model add(index 1) 0)))))

Def display-bit Fn {flags: u8 bit: s64}
  eq(rem(div(to-s64(flags) bit) 2) 1)

DefType viewport {width: u16 height: u16 host-switch: bool}

Def display-width Fn {width: u16} to-s64(max(128 width))
Def display-height Fn {height: u16} to-s64(max(144 height))
Def display-wide Fn {w: s64} ge(w 220)
Def display-header-h Fn {w: s64} If display-wide(w) 40 25
Def display-footer-h Fn {w: s64} If display-wide(w) 34 25
Def display-menu-row-h Fn {w: s64} If display-wide(w) 34 23
Def display-body-h Fn {w: s64} If display-wide(w) 31 22
Def display-font-size Fn {w: s64 size: u8}
  If display-wide(w) Match size [(1 36) (2 48) (_ 28)]
    If eq(size 2) 32 20

Def display-available-glyph-width Fn {glyph: string}
  If lt(glyph " ") 320
  If gt(glyph "~") 320
    If lt(glyph "O")
      If lt(glyph "7")
        If lt(glyph "+")
          If lt(glyph "%")
            If lt(glyph "\"")
              If lt(glyph "!") 86 86
              If lt(glyph "#")
                125
                If lt(glyph "$") 225 199
            If lt(glyph "(")
              If lt(glyph "&")
                270
                If lt(glyph "'") 220 67
              If lt(glyph ")")
                108
                If lt(glyph "*") 108 128
          If lt(glyph "1")
            If lt(glyph ".")
              If lt(glyph ",")
                186
                If lt(glyph "-") 73 123
              If lt(glyph "/")
                73
                If lt(glyph "0") 113 213
            If lt(glyph "4")
              If lt(glyph "2")
                118
                If lt(glyph "3") 184 183
              If lt(glyph "5")
                214
                If lt(glyph "6") 184 197
        If lt(glyph "C")
          If lt(glyph "=")
            If lt(glyph ":")
              If lt(glyph "8")
                191
                If lt(glyph "9") 206 197
              If lt(glyph ";")
                73
                If lt(glyph "<") 73 186
            If lt(glyph "@")
              If lt(glyph ">")
                186
                If lt(glyph "?") 186 183
              If lt(glyph "A")
                331
                If lt(glyph "B") 234 242
          If lt(glyph "I")
            If lt(glyph "F")
              If lt(glyph "D")
                231
                If lt(glyph "E") 264 214
              If lt(glyph "G")
                203
                If lt(glyph "H") 247 260
            If lt(glyph "L")
              If lt(glyph "J")
                99
                If lt(glyph "K") 164 230
              If lt(glyph "M")
                190
                If lt(glyph "N") 306 260
      If lt(glyph "g")
        If lt(glyph "[")
          If lt(glyph "U")
            If lt(glyph "R")
              If lt(glyph "P")
                269
                If lt(glyph "Q") 231 269
              If lt(glyph "S")
                233
                If lt(glyph "T") 199 188
            If lt(glyph "X")
              If lt(glyph "V")
                253
                If lt(glyph "W") 228 360
              If lt(glyph "Y")
                215
                If lt(glyph "Z") 207 210
          If lt(glyph "a")
            If lt(glyph "^")
              If lt(glyph "\\")
                107
                If lt(glyph "]") 113 107
              If lt(glyph "_")
                187
                If lt(glyph "`") 160 192
            If lt(glyph "d")
              If lt(glyph "b")
                191
                If lt(glyph "c") 218 183
              If lt(glyph "e")
                218
                If lt(glyph "f") 196 113
        If lt(glyph "s")
          If lt(glyph "m")
            If lt(glyph "j")
              If lt(glyph "h")
                221
                If lt(glyph "i") 218 89
              If lt(glyph "k")
                91
                If lt(glyph "l") 197 89
            If lt(glyph "p")
              If lt(glyph "n")
                338
                If lt(glyph "o") 218 203
              If lt(glyph "q")
                218
                If lt(glyph "r") 218 131
          If lt(glyph "y")
            If lt(glyph "v")
              If lt(glyph "t")
                160
                If lt(glyph "u") 132 217
              If lt(glyph "w")
                179
                If lt(glyph "x") 288 177
            If lt(glyph "|")
              If lt(glyph "z")
                179
                If lt(glyph "{") 167 112
              If lt(glyph "}")
                96
                If lt(glyph "~") 112 186

Def display-width-units Fn {text: string}
  If eq(len(text) 1) display-available-glyph-width(text)
    fold(Fn {total: s64 glyph: string}
      If eq(len(glyph) 0) total add(total display-available-glyph-width(glyph))
      0 split(text ""))

Def display-text-width Fn {text: string viewport: s64 size: u8}
  div(add(mul(display-width-units(text) display-font-size(viewport size)) 319) 320)

Def display-centered Fn {frame: display-frame x: s64 w: s64 y: s64 color: u8 size: u8 text: string viewport: s64}
  Let {estimate: display-text-width(text viewport size)}
    display-text(frame add(x max(0 div(sub(w estimate) 2))) y color size text)

Def display-line Fn {frame: display-frame x: s64 y: s64 end-x: s64 end-y: s64 color: u8 width: u8}
  Match frame [({commands: commands labels: labels}
    {commands: push(commands {opcode: 5 x: to-u16(x) y: to-u16(y) w: to-u16(end-x) h: to-u16(end-y)
                              first: color second: width third: 0})
     labels: labels})]

Def display-arc Fn {frame: display-frame x: s64 y: s64 radius: u16 start: u16 sweep: u8 color: u8 width: u8}
  Match frame [({commands: commands labels: labels}
    {commands: push(commands {opcode: 6 x: to-u16(x) y: to-u16(y) w: radius h: start
                              first: color second: width third: sweep})
     labels: labels})]

Def display-home-battery Fn {frame: display-frame mid: s64 y: s64 w: s64 level: u8 active: bool}
  Let {wide: display-wide(w)
       width: If wide 64 40
       height: If wide 31 23
       x: sub(mid div(width 2))
       color: If active The u8 5 The u8 6
       label-w: If wide 112 72
       label-x: max(1 sub(mid div(label-w 2)))
       reading: If active If eq(level 255) "?" str-cat(to-string(min(level 100)) "%") "Offline"
       frame: display-box(frame 2 x y width height color 4)
       frame: display-box(frame 1 add(x width) add(y div(height 3)) 4 div(height 3) color 1)
       frame: If active
         If eq(level 255)
           display-box(frame 1 sub(mid 4) add(y div(height 2)) 8 2 color 1)
         If eq(level 0) frame
           display-box(frame 1 add(x 4) add(y 4) max(1 div(mul(sub(width 8) to-s64(min(level 100))) 100))
             sub(height 8) color 2)
         display-line(frame add(x 5) add(y 4) add(x sub(width 6)) add(y sub(height 5)) color 3)}
    display-centered(frame label-x label-w add(y add(height 5)) 1 0 reading w)

Def display-home-signal Fn {frame: display-frame mid: s64 y: s64 connected: bool}
  If connected
    Let {center: add(y 25)
         frame: display-arc(frame mid center 24 225 90 5 3)
         frame: display-arc(frame mid center 17 225 90 5 3)
         frame: display-arc(frame mid center 10 225 90 5 3)}
      display-box(frame 1 sub(mid 2) add(y 23) 5 5 5 3)
    Let {left: sub(mid 9)
         right: add(mid 9)
         top: add(y 4)
         bottom: add(y 22)
         frame: display-line(frame left top right bottom 6 3)}
      display-line(frame right top left bottom 6 3)

Def display-target-label Fn {target: u8}
  Match target [
    (0 "Prospector") (1 "BT 1") (2 "BT 2") (3 "BT 3") (4 "BT 4") (5 "BT 5") (6 "USB")
    (_ "No target")]

Def display-home Fn {w: s64 h: s64 model: list(u8)}
  Let {flags: display-get(model 7 0)
       wide: display-wide(w)
       selection: display-get(model 15 255)
       target: display-target-label(selection)
       connection: display-get(model 16 0)
       status: Match connection [(1 "Offline") (2 "Connecting") (3 "Connected") (_ "Unknown")]
       battery-y: sub(h If wide 86 56)
       mid: div(w 2)
       left: If wide add(div(w 5) 4) div(w 4)
       right: sub(w left)
       frame: If gt(selection 6)
         display-centered(display-empty 0 w If wide 37 27 1 0 If wide "Awaiting Toucan" "No target" w)
         Let {frame: display-centered(display-empty 0 w If wide 20 12 1 1 target w)}
           display-centered(frame 0 w If wide 62 38 1 0 status w)
       frame: display-home-signal(frame mid If wide 95 67 eq(connection 3))
       frame: display-home-battery(frame left battery-y w display-get(model 8 255) display-bit(flags 1))}
    display-home-battery(frame right battery-y w display-get(model 9 255) display-bit(flags 2))

DefType display-wrapped {lines: list(string) pending: string used: s64}

Def display-wrap Fn {text: string maximum: s64 viewport: s64}
  Let {font: display-font-size(viewport 0)
       units: div(mul(maximum 320) font)
       start: The display-wrapped {lines: [] pending: "" used: 0}
       wrapped: fold(Fn {acc: display-wrapped glyph: string}
         Match acc [({lines: lines pending: pending used: used}
           If eq(len(glyph) 0) acc
             Let {width: display-available-glyph-width(glyph)
                  next: add(used width)}
               If If gt(next units) gt(used 0) false
                 {lines: push(lines pending) pending: glyph used: width}
                 {lines: lines pending: str-cat(pending glyph) used: next})]
         start split(text ""))}
    Match wrapped [({lines: lines pending: pending used: used} push(lines pending))]

Def display-compact-text Fn {text: string w: s64}
  If le(display-text-width(text w 3) sub(w 16)) text
    str-cat(get(display-wrap(text sub(w 34) w) 0) "...")

Def display-anchored-top Fn {top: s64 bottom: s64 center: s64 height: s64}
  max(top min(sub(center div(height 2)) sub(bottom height)))

Def display-line-list Fn {frame: display-frame w: s64 first: s64 step: s64
                         lines: list(string) visible: s64}
  fold(Fn {acc: display-frame row: s64}
    Let {tail: If eq(row sub(visible 1)) gt(len(lines) visible) false
         line: If tail str-cat(get(lines row) "...") get(lines row)}
      display-centered(acc 0 w add(first mul(row step)) 1 0 line w)
    frame range(0 visible))

Def display-line-text Fn {frame: display-frame w: s64 top: s64 bottom: s64 center: s64
                         text: string body-width: s64}
  Let {available: sub(bottom top)
       body-height: If display-wide(w) 30 22
       large-height: If display-wide(w) 52 35
       max-width: sub(w 16)}
    If If ge(available large-height) le(display-text-width(text w 2) max-width) false
      display-centered(frame 0 w display-anchored-top(top bottom center large-height) 1 2 text w)
    If le(body-width max-width)
      display-centered(frame 0 w display-anchored-top(top bottom center body-height) 1 0 text w)
      Let {lines: display-wrap(text sub(w 28) w)
           gap: 2
           step: add(body-height gap)
           visible: min(len(lines) max(1 div(add(available gap) step)))
           height: sub(mul(visible step) gap)
           first: display-anchored-top(top bottom center height)}
        display-line-list(frame w first step lines visible)

Def display-typing Fn {w: s64 h: s64 model: list(u8) detail: string}
  Let {wide: display-wide(w)
       large-height: If wide 52 35
       large-descent: If wide 9 6
       body-height: If wide 30 22
       body-descent: If wide 5 4
       margin: 8
       gap: 6
       line-gap: 2
       body-width: If eq(len(detail) 0) 0 display-text-width(detail w 0)
       wrapped: gt(body-width sub(w 16))
       count: min(7 to-s64(display-get(model 10 0)))
       buffer-h: If wide 22 18
       buffer-y: sub(h add(buffer-h margin))
       bottom: If eq(count 0) sub(h margin) sub(buffer-y gap)
       ideal-dot-h: If wrapped sub(div(h 3) 12) sub(div(h 2) 8)
       minimum-preview: If wrapped body-height large-height
       maximum-dot-h: sub(bottom add(add(margin gap) minimum-preview))
       dot-h: mul(div(min(ideal-dot-h maximum-dot-h) 4) 4)
       dot-w: div(dot-h 2)
       x: div(sub(w dot-w) 2)
       step: add(body-height line-gap)
       lines: If wrapped display-wrap(detail sub(w 28) w) The list(string) []
       available: sub(bottom add(add(margin dot-h) gap))
       visible: min(len(lines) div(add(available line-gap) step))
       preview-h: If wrapped sub(mul(visible step) line-gap) large-height
       descent: If wrapped body-descent large-descent
       block-h: add(add(dot-h gap) preview-h)
       dot-y: max(margin min(div(sub(h sub(block-h descent)) 2) sub(bottom block-h)))
       preview-y: add(add(dot-y dot-h) gap)
       held: display-get-wide(model 3)
       frame: display-dot-grid(display-empty x dot-y dot-w dot-h held true)
       frame: If eq(count 0) frame
         Let {stride: min(21 div(sub(w 12) count)) start: div(sub(w mul(stride count)) 2)}
           fold(Fn {acc: display-frame index: s64}
             display-dots(acc add(start mul(stride index)) buffer-y
               min(15 sub(stride 2)) buffer-h display-get-wide(model add(15 mul(index 2))))
             frame range(0 count))}
    If eq(len(detail) 0) frame
    If wrapped display-line-list(frame w preview-y step lines visible)
    If le(display-text-width(detail w 2) sub(w 16))
      display-centered(frame 0 w preview-y 1 2 detail w)
      Let {inset: div(sub(sub(large-height large-descent) sub(body-height body-descent)) 2)}
        display-centered(frame 0 w add(preview-y inset) 1 0 detail w)

Def display-section-count The s64 8

Def display-section-title Fn {section: s64 wide: bool}
  Match section [
    (0 "Lowercase")
    (1 "Capitals")
    (2 "Punctuation")
    (3 "Digits")
    (4 "Navigation")
    (5 "Editing")
    (6 If wide "UEB Wordsigns" "Wordsigns")
    (7 If wide "UEB Shortforms" "Shortforms")
    (_ "Reference")]

Def display-letter-cell Fn {index: s64}
  get(The list(u16) [1 3 9 25 17 11 27 19 10 26 5 7 13 29 21 15 31 23 14 30 37 39 58 45 61 53] index)

Def display-entry-row Fn {label: string cells: list(u16)}
  The display-entry {label: label cells: cells mnemonic: ""}

Def display-contraction-row Fn {label: string cells: list(u16) mnemonic: string}
  The display-entry {label: label cells: cells mnemonic: mnemonic}

Def display-section-length Fn {section: s64}
  Match section [(0 26) (1 26) (2 32) (3 10) (4 8) (5 14) (6 23) (7 75) (_ 0)]

Def display-symbol Fn {index: s64 ueb: bool}
  Match index [
    (0 display-entry-row("!" If ueb [22] [46]))
    (1 display-entry-row("\"" If ueb [32 54] [16]))
    (2 display-entry-row("#" If ueb [56 57] [60]))
    (3 display-entry-row("$" If ueb [8 14] [43]))
    (4 display-entry-row("%" If ueb [40 52] [41]))
    (5 display-entry-row("&" If ueb [8 47] [47]))
    (6 display-entry-row("'" If ueb [4] [4]))
    (7 display-entry-row("(" If ueb [16 35] [55]))
    (8 display-entry-row(")" If ueb [16 28] [62]))
    (9 display-entry-row("*" If ueb [16 20] [33]))
    (10 display-entry-row("+" If ueb [16 22] [44]))
    (11 display-entry-row("," If ueb [2] [32]))
    (12 display-entry-row("-" If ueb [36] [36]))
    (13 display-entry-row("." If ueb [50] [40]))
    (14 display-entry-row("/" If ueb [56 12] [12]))
    (15 display-entry-row(":" If ueb [18] [49]))
    (16 display-entry-row(";" If ueb [6] [48]))
    (17 display-entry-row("<" If ueb [8 35] [35]))
    (18 display-entry-row(">" If ueb [8 28] [28]))
    (19 display-entry-row("=" If ueb [16 54] [63]))
    (20 display-entry-row("?" If ueb [48 38] [57]))
    (21 display-entry-row("@" If ueb [8 1] [72]))
    (22 display-entry-row("[" If ueb [40 35] [106]))
    (23 display-entry-row("]" If ueb [40 28] [123]))
    (24 display-entry-row("\\" If ueb [56 33] [115]))
    (25 display-entry-row("^" If ueb [8 34] [88]))
    (26 display-entry-row("_" If ueb [40 36] [56]))
    (27 display-entry-row("`" If ueb [40 33] [8]))
    (28 display-entry-row("{" If ueb [56 35] [42]))
    (29 display-entry-row("}" If ueb [56 28] [59]))
    (30 display-entry-row("|" If ueb [56 51] [51]))
    (31 display-entry-row("~" If ueb [8 20] [24]))
    (_ display-entry-row("" []))]

Def display-wordsign Fn {index: s64}
  Match index [
    (0 display-contraction-row("as" [53] "z"))
    (1 display-contraction-row("but" [3] "b"))
    (2 display-contraction-row("can" [9] "c"))
    (3 display-contraction-row("do" [25] "d"))
    (4 display-contraction-row("every" [17] "e"))
    (5 display-contraction-row("from" [11] "f"))
    (6 display-contraction-row("go" [27] "g"))
    (7 display-contraction-row("have" [19] "h"))
    (8 display-contraction-row("it" [45] "x"))
    (9 display-contraction-row("just" [26] "j"))
    (10 display-contraction-row("knowledge" [5] "k"))
    (11 display-contraction-row("like" [7] "l"))
    (12 display-contraction-row("more" [13] "m"))
    (13 display-contraction-row("not" [29] "n"))
    (14 display-contraction-row("people" [15] "p"))
    (15 display-contraction-row("quite" [31] "q"))
    (16 display-contraction-row("rather" [23] "r"))
    (17 display-contraction-row("so" [14] "s"))
    (18 display-contraction-row("that" [30] "t"))
    (19 display-contraction-row("us" [37] "u"))
    (20 display-contraction-row("very" [39] "v"))
    (21 display-contraction-row("will" [58] "w"))
    (22 display-contraction-row("you" [61] "y"))
    (_ display-entry-row("" []))]

Def display-shortform Fn {index: s64}
  Match index [
    (0 display-contraction-row("about" [1 3] "ab"))
    (1 display-contraction-row("above" [1 3 39] "abv"))
    (2 display-contraction-row("according" [1 9] "ac"))
    (3 display-contraction-row("across" [1 9 23] "acr"))
    (4 display-contraction-row("after" [1 11] "af"))
    (5 display-contraction-row("afternoon" [1 11 29] "afn"))
    (6 display-contraction-row("afterward" [1 11 58] "afw"))
    (7 display-contraction-row("again" [1 27] "ag"))
    (8 display-entry-row("against" [1 27 12]))
    (9 display-contraction-row("almost" [1 7 13] "alm"))
    (10 display-contraction-row("already" [1 7 23] "alr"))
    (11 display-contraction-row("also" [1 7] "al"))
    (12 display-entry-row("although" [1 7 57]))
    (13 display-contraction-row("altogether" [1 7 30] "alt"))
    (14 display-contraction-row("always" [1 7 58] "alw"))
    (15 display-entry-row("because" [6 9]))
    (16 display-entry-row("before" [6 11]))
    (17 display-entry-row("behind" [6 19]))
    (18 display-entry-row("below" [6 7]))
    (19 display-entry-row("beneath" [6 29]))
    (20 display-entry-row("beside" [6 14]))
    (21 display-entry-row("between" [6 30]))
    (22 display-entry-row("beyond" [6 61]))
    (23 display-contraction-row("blind" [3 7] "bl"))
    (24 display-contraction-row("braille" [3 23 7] "brl"))
    (25 display-entry-row("children" [33 29]))
    (26 display-entry-row("conceive" [18 9 39]))
    (27 display-entry-row("conceiving" [18 9 39 27]))
    (28 display-contraction-row("could" [9 25] "cd"))
    (29 display-contraction-row("deceive" [25 9 39] "dcv"))
    (30 display-contraction-row("deceiving" [25 9 39 27] "dcvg"))
    (31 display-contraction-row("declare" [25 9 7] "dcl"))
    (32 display-contraction-row("declaring" [25 9 7 27] "dclg"))
    (33 display-contraction-row("either" [17 10] "ei"))
    (34 display-entry-row("first" [11 12]))
    (35 display-contraction-row("friend" [11 23] "fr"))
    (36 display-contraction-row("good" [27 25] "gd"))
    (37 display-contraction-row("great" [27 23 30] "grt"))
    (38 display-entry-row("herself" [19 59 11]))
    (39 display-contraction-row("him" [19 13] "hm"))
    (40 display-contraction-row("himself" [19 13 11] "hmf"))
    (41 display-contraction-row("immediate" [10 13 13] "imm"))
    (42 display-contraction-row("its" [45 14] "xs"))
    (43 display-contraction-row("itself" [45 11] "xf"))
    (44 display-contraction-row("letter" [7 23] "lr"))
    (45 display-contraction-row("little" [7 7] "ll"))
    (46 display-entry-row("much" [13 33]))
    (47 display-entry-row("must" [13 12]))
    (48 display-contraction-row("myself" [13 61 11] "myf"))
    (49 display-contraction-row("necessary" [29 17 9] "nec"))
    (50 display-contraction-row("neither" [29 17 10] "nei"))
    (51 display-entry-row("oneself" [16 21 11]))
    (52 display-entry-row("ourselves" [51 23 39 14]))
    (53 display-contraction-row("paid" [15 25] "pd"))
    (54 display-entry-row("perceive" [15 59 9 39]))
    (55 display-entry-row("perceiving" [15 59 9 39 27]))
    (56 display-entry-row("perhaps" [15 59 19]))
    (57 display-contraction-row("quick" [31 5] "qk"))
    (58 display-contraction-row("receive" [23 9 39] "rcv"))
    (59 display-contraction-row("receiving" [23 9 39 27] "rcvg"))
    (60 display-contraction-row("rejoice" [23 26 9] "rjc"))
    (61 display-contraction-row("rejoicing" [23 26 9 27] "rjcg"))
    (62 display-contraction-row("said" [14 25] "sd"))
    (63 display-entry-row("should" [41 25]))
    (64 display-entry-row("such" [14 33]))
    (65 display-entry-row("themselves" [46 13 39 14]))
    (66 display-entry-row("thyself" [57 61 11]))
    (67 display-contraction-row("today" [30 25] "td"))
    (68 display-contraction-row("together" [30 27 23] "tgr"))
    (69 display-contraction-row("tomorrow" [30 13] "tm"))
    (70 display-contraction-row("tonight" [30 29] "tn"))
    (71 display-contraction-row("would" [58 25] "wd"))
    (72 display-contraction-row("your" [61 23] "yr"))
    (73 display-contraction-row("yourself" [61 23 11] "yrf"))
    (74 display-contraction-row("yourselves" [61 23 39 14] "yrvs"))
    (_ display-entry-row("" []))]

Def display-navigation-entry Fn {index: s64}
  Match index [
    (0 display-entry-row("Left" [257]))
    (1 display-entry-row("Right" [264]))
    (2 display-entry-row("Up" [260]))
    (3 display-entry-row("Down" [288]))
    (4 display-entry-row("Home" [263]))
    (5 display-entry-row("End" [312]))
    (6 display-entry-row("Page Up" [316]))
    (7 display-entry-row("Page Down" [313]))
    (_ display-entry-row("" []))]

Def display-editing-entry Fn {index: s64}
  Match index [
    (0 display-entry-row("Space" [256]))
    (1 display-entry-row("Tab" [286]))
    (2 display-entry-row("Return" [273]))
    (3 display-entry-row("Escape" [259]))
    (4 display-entry-row("Backspace" [281]))
    (5 display-entry-row("Backspace" [64]))
    (6 display-entry-row("Return" [128]))
    (7 display-entry-row("Return" [320]))
    (8 display-entry-row("Return" [384]))
    (9 display-entry-row("Shift" [328]))
    (10 display-entry-row("Control" [324]))
    (11 display-entry-row("Option" [322]))
    (12 display-entry-row("Command" [321]))
    (13 display-entry-row("Globe" [336]))
    (_ display-entry-row("" []))]

Def display-reference-entry Fn {section: s64 index: s64 ueb: bool}
  Match section [
    (0 display-entry-row(str-cat(to-char(add(97 index))) [display-letter-cell(index)]))
    (1 display-entry-row(str-cat(to-char(add(65 index)))
         If ueb [32 display-letter-cell(index)] [to-u16(add(64 to-s64(display-letter-cell(index))))]))
    (2 display-symbol(index ueb))
    (3 display-entry-row(str-cat(to-char(add(48 index)))
         If ueb [60 get(The list(u16) [26 1 3 9 25 17 11 27 19 10] index)]
           [get(The list(u16) [52 2 6 18 50 34 22 54 38 20] index)]))
    (4 display-navigation-entry(index))
    (5 display-editing-entry(index))
    (6 display-wordsign(index))
    (7 display-shortform(index))
    (_ display-entry-row("" []))]

Def display-mnemonic Fn {cell: u16}
  Match rem(to-s64(cell) 256) [
    (1 "a") (3 "b") (9 "c") (25 "d") (17 "e") (11 "f")
    (27 "g") (19 "h") (10 "i") (26 "j") (5 "k") (7 "l")
    (13 "m") (29 "n") (21 "o") (15 "p") (31 "q") (23 "r")
    (14 "s") (30 "t") (37 "u") (39 "v") (58 "w") (45 "x")
    (61 "y") (53 "z")
    (_ "")]

Def display-reference-cols Fn {section: s64 wide: bool}
  If lt(section 4) If wide 4 3 1

Def display-reference-row-h Fn {section: s64 w: s64}
  If display-wide(w) 35 If ge(section 6) 44 24

Def display-row-space Fn {w: s64 h: s64}
  sub(h add(add(display-header-h(w) display-footer-h(w)) 6))

Def display-reference-rows Fn {section: s64 w: s64 h: s64}
  If lt(section 4) 4
    Let {legend: If ge(section 6) display-footer-h(w) 0}
      max(1 min(6 div(sub(display-row-space(w h) legend) display-reference-row-h(section w))))

Def display-reference-page Fn {section: s64 w: s64 h: s64}
  mul(display-reference-cols(section display-wide(w)) display-reference-rows(section w h))

Def display-selection Fn {frame: display-frame x: s64 y: s64 w: s64 h: s64 wide: bool}
  display-box(frame If wide The u8 1 The u8 2 x y w h 8 3)

Def display-reference-row Fn {frame: display-frame section: s64 index: s64
                              x: s64 y: s64 w: s64 h: s64 ueb: bool selected: bool viewport: s64}
  Let {wide: display-wide(viewport)
       frame: If selected display-selection(frame x y w sub(h 1) wide)
                 If eq(rem(index 2) 0) display-box(frame 1 x y w sub(h 1) 7 2) frame}
    Match display-reference-entry(section index ueb) [({label: label cells: cells mnemonic: mnemonic}
      Let {compact: lt(section 4)
           stacked: If wide false ge(section 6)
           count: len(cells)
           glyph-width: display-text-width(label viewport 0)
           body-h: display-body-h(viewport)
           label-y: If stacked y add(y max(0 div(sub(h body-h) 2)))
           body-y: If stacked add(y body-h) label-y
           slot: div(w 2)
           label-x: If compact add(x max(0 div(sub(slot glyph-width) 2))) add(x 3)
           left: If compact slot If stacked 3 add(glyph-width 9)
           right-margin: If compact 4 6
           stride: max(8 min(If wide 22 14 div(sub(sub(w left) right-margin) max(count 1))))
           grid-width: mul(count stride)
           start: If compact add(add(x slot) div(sub(slot grid-width) 2)) sub(add(x w) add(grid-width 5))
           cell-w: sub(stride 1)
           dot-h: min(If wide 32 21 sub(h 1))
           dot-y: If stacked body-y add(y max(0 div(sub(h dot-h) 2)))
           frame: display-text(frame label-x label-y 1 0 label)}
        If gt(len(mnemonic) 0)
          display-text(frame sub(add(x w) add(display-text-width(mnemonic viewport 0) 5)) body-y 3 0 mnemonic)
          fold(Fn {acc: display-frame offset: s64}
            Let {cell: get(cells offset)
                 cell-x: add(start mul(offset stride))
                 letter: If ge(section 7) display-mnemonic(cell) ""}
              If gt(len(letter) 0)
                display-centered(acc cell-x stride body-y 3 0 letter viewport)
                display-dots(acc cell-x dot-y cell-w dot-h cell)
            frame range(0 count)))]

Def display-bar Fn {frame: display-frame w: s64 h: s64 screen: u8}
  Let {bar-h: display-footer-h(w)
       y: sub(h bar-h)
       wide: display-wide(w)
       frame: display-box(frame 1 0 y w bar-h 7 0)
       editor: If eq(screen 11) true eq(screen 12)
       prefix: display-text-width("(H)elp (" w 3)
       dot-w: If wide 15 11
       glyph-gap: If wide 6 4
       help-w: add(add(prefix dot-w) add(display-text-width(")" w 3) mul(2 glyph-gap)))
       gap: If wide 20 3
       tip: Match screen [
         (4 "N/P")
         (5 If wide "Esc Back" "")
         (11 If wide "Esc Back" "")
         (12 If wide "Save" "")
         (13 "N/P")
         (_ "")]
       tip-w: Match tip [
         ("N/P" If wide 54 38)
         ("Esc Back" 121)
         ("Save" 70)
         (_ 0)]
       show-tip: If gt(tip-w 0) le(add(add(help-w gap) tip-w) sub(w If wide 36 2)) false
       total: If show-tip add(add(help-w gap) tip-w) help-w
       start: max(1 div(sub(w total) 2))
       x: add(add(start prefix) glyph-gap)
       frame: display-text(frame start add(y 2) 1 3 "(H)elp (")
       frame: display-dots(frame x add(y 3) dot-w sub(bar-h 5) If editor The u16 275 The u16 19)
       frame: display-text(frame add(x add(dot-w glyph-gap)) add(y 2) 1 3 ")")}
    If show-tip display-text(frame add(start add(help-w gap)) add(y 2) 1 3 tip) frame

Def display-heading Fn {frame: display-frame w: s64 title: string}
  Let {wide: display-wide(w)
       compact: If wide gt(display-text-width(title w 1) sub(w 36)) false
       size: If compact The u8 0 The u8 1}
    display-centered(display-box(frame 1 0 0 w display-header-h(w) 3 0)
      0 w If compact 4 1 9 size title w)

Def display-grid Fn {w: s64 h: s64 model: list(u8) selected: s64}
  Let {section: div(selected 256)
       index: rem(selected 256)
       cols: display-reference-cols(section display-wide(w))
       rows: display-reference-rows(section w h)
       page: display-reference-page(section w h)
       start: mul(div(index page) page)
       count: min(page max(0 sub(display-section-length(section) start)))
       cell-w: div(sub(w 8) cols)
       grid-y: add(display-header-h(w) 3)
       cell-h: If lt(section 4) div(sub(h add(add(grid-y display-footer-h(w)) 5)) rows)
         display-reference-row-h(section w)
       frame: display-heading(display-empty w display-section-title(section display-wide(w)))
       frame: If ge(section 6)
         display-text(frame 7 sub(h mul(2 display-footer-h(w))) 2 3 If display-wide(w) "Reference only" "Guide only") frame
       frame: fold(Fn {acc: display-frame offset: s64}
         Let {row: div(offset cols) col: rem(offset cols) i: add(start offset)}
           display-reference-row(acc section i
             add(4 mul(col cell-w)) add(grid-y mul(row cell-h)) sub(cell-w 1) cell-h
             display-bit(display-get(model 7 0) 8) eq(i index) w)
         frame range(0 count))}
    display-bar(frame w h 4)

Def display-detail Fn {w: s64 h: s64 model: list(u8) selected: s64}
  Let {section: min(div(selected 256) sub(display-section-count 1))
       index: min(rem(selected 256) sub(display-section-length(section) 1))
       frame: display-heading(display-empty w display-section-title(section display-wide(w)))}
    Match display-reference-entry(section index display-bit(display-get(model 7 0) 8)) [
      ({label: label cells: cells mnemonic: mnemonic}
        Let {count: max(1 len(cells))
             stride: min(37 div(sub(w 16) count))
             start: div(sub(w mul(stride count)) 2)
             top: max(58 sub(div(h 2) 14))
             frame: display-centered(frame 0 w add(display-header-h(w) 6) 1 0 label w)
             frame: fold(Fn {acc: display-frame offset: s64}
               Let {cell: get(cells offset) x: add(start mul(stride offset))
                    mnemonic: display-mnemonic(cell)
                    acc: display-dots(acc x top sub(stride 4) 42 cell)}
                 If eq(len(mnemonic) 0) acc
                   display-text(acc add(x div(stride 3)) add(top 44)
                     If ge(cell 256) The u8 4 The u8 3 3 mnemonic)
               frame range(0 len(cells)))}
          display-bar(frame w h 5))]

Def display-menu-count Fn {screen: u8 host-switch: bool}
  Match screen [(2 If host-switch 5 4) (3 display-section-count) (6 If host-switch 5 4)
    (7 3) (8 12) (10 6) (14 4) (15 7) (17 3) (_ 0)]

Def display-menu-title Fn {screen: u8 w: s64}
  Match screen [
    (2 "Braille Menu")
    (3 "Reference")
    (6 "Main Menu")
    (7 If display-wide(w) "Configuration" "Settings")
    (8 "Bindings")
    (9 "Debug")
    (10 "Edit Binding")
    (11 If display-wide(w) "Capture Chord" "Capture")
    (12 "Expansion")
    (13 "Help")
    (15 "Host Target")
    (17 "Prospector")
    (_ "")]

Def display-menu-label Fn {screen: u8 index: s64 ueb: bool wide: bool}
  Match screen [
    (3 display-section-title(index wide))
    (7 Match index [
      (0 If ueb "UEB Grade 1" "Computer 8")
      (1 "Reconnect")
      (2 If wide "Clear pairings" "Clear bonds")
      (_ "")])
    (8 str-cat("Slot " to-string(add(index 1))))
    (10 Match index [
      (0 "One chord")
      (1 "Two chords")
      (2 "Three chords")
      (3 "Four chords")
      (4 If wide "Edit expansion" "Edit text")
      (5 If wide "Delete binding" "Delete")
      (_ "")])
    (15 display-target-label(to-u8(index)))
    (17 get(The list(string) ["Reference" "Debug" "Touch Help"] index))
    (_ Match index [
      (0 "(R)eference")
      (1 If wide "(C)onfiguration" "(C)onfig")
      (2 "(B)indings")
      (3 "(D)ebug")
      (4 "(T)arget")
      (_ "")])]

Def display-menu-preview-h Fn {screen: u8} If eq(screen 8) 50 0

Def display-menu-list-row-h Fn {screen: u8 w: s64 h: s64 host-switch: bool}
  If If eq(screen 2) true eq(screen 6)
    If If display-wide(w) true host-switch
      div(display-row-space(w h) display-menu-count(screen host-switch))
      display-menu-row-h(w)
    display-menu-row-h(w)

Def display-menu-rows Fn {screen: u8 w: s64 h: s64 host-switch: bool}
  Let {available: sub(display-row-space(w h) display-menu-preview-h(screen))}
    max(1 min(If eq(screen 15) 4 8 div(available display-menu-list-row-h(screen w h host-switch))))

Def display-menu-list Fn {frame: display-frame screen: u8 w: s64 h: s64 selected: s64 flags: u8}
  Let {host-switch: display-bit(flags 32)
       rows: display-menu-rows(screen w h host-switch)
       start: mul(div(selected rows) rows)
       count: min(rows max(0 sub(display-menu-count(screen host-switch) start)))
       wide: display-wide(w)
       top: add(display-header-h(w) 3)
       row-h: display-menu-list-row-h(screen w h host-switch)
       text-offset: div(sub(row-h display-body-h(w)) 2)}
    fold(Fn {acc: display-frame offset: s64}
      Let {y: add(top mul(offset row-h))
           index: add(start offset)
           acc: If eq(index selected) display-selection(acc 1 y sub(w 2) sub(row-h 1) wide)
                  If eq(rem(index 2) 0) display-box(acc 1 1 y sub(w 2) sub(row-h 1) 7 2) acc}
        display-text(acc 8 add(y text-offset) 1 0 display-menu-label(screen index display-bit(flags 8) wide))
      frame range(0 count))

Def display-notice Fn {notice: u8}
  Match notice [
    (1 "Applied")
    (2 "Deleted")
    (3 "Busy")
    (4 "Invalid text")
    (5 "Text is full")
    (6 "Enter expansion")
    (7 "Trigger is in use")
    (8 "Reserved chord")
    (9 "Empty slot")
    (10 "Radio requested")
    (_ "")]

Def display-triggers Fn {frame: display-frame w: s64 y: s64 model: list(u8)}
  Let {offset: add(15 mul(2 min(to-s64(display-get(model 10 0)) 7)))
       count: min(to-s64(display-get(model offset 0)) 4)}
    If eq(count 0) frame
      Let {stride: min(28 div(sub(w 12) count))
           start: div(sub(w mul(count stride)) 2)}
        fold(Fn {acc: display-frame index: s64}
          display-dots(acc add(start mul(index stride)) y min(19 sub(stride 2)) 30
            display-get-wide(model add(add(offset 1) mul(2 index))))
          frame range(0 count))

Def display-bootloader Fn {w: s64 h: s64 model: list(u8) selected: s64}
  Let {wide: display-wide(w)
       frame: display-heading(display-empty w If wide "UF2 Bootloader" "UF2 Loader")
       top: add(display-header-h(w) 3)
       row-h: If wide 32 22
       available: display-get(model 15 0)
       devices: display-get(model 16 0)
       frame: fold(Fn {acc: display-frame index: s64}
         Let {y: add(top mul(index row-h))
              acc: If eq(index selected) display-selection(acc 1 y sub(w 2) sub(row-h 1) wide) acc
              label: get(The list(string) ["Left half" "Right half" "Prospector" "Enter UF2"] index)
              text-y: add(y div(sub(row-h display-body-h(w)) 2))
              acc: display-text(acc 8 text-y 1 0 label)}
           If eq(index 3) acc
             Let {device: get(The list(s64) [1 2 4] index)
                  online: display-bit(available device)
                  chosen: display-bit(devices device)
                  status: If chosen "Selected" If online "Ready" "Unavailable"
                  marker-x: sub(w 22)
                  marker-y: add(y 3)}
               If wide display-text(acc sub(w add(8 display-text-width(status w 3))) text-y 1 3 status)
               If online
                 Let {acc: display-box(acc 2 marker-x marker-y 16 16 1 2)}
                   If chosen display-box(acc 1 add(marker-x 4) add(marker-y 4) 8 8 1 1) acc
                 display-text(acc add(marker-x 2) text-y 1 3 "X")
         frame range(0 4))
       notice: Match display-get(model 17 0) [
         (1 If wide "Select devices first" "Pick devices")
         (2 If wide "Device unavailable" "Unavailable")
         (3 "UF2 requested")
         (4 If wide "Some requests failed" "Some failed")
         (5 If wide "UF2 request failed" "Request failed")
         (_ "")]
       frame: If eq(len(notice) 0) frame
         display-text(frame 8 sub(h add(add(display-footer-h(w) display-body-h(w)) 3)) 1 3 notice)}
    display-bar(frame w h 14)

Def display-menu Fn {screen: u8 w: s64 h: s64 model: list(u8) selected: s64 detail: string}
  Let {flags: display-get(model 7 0)
       wide: display-wide(w)
       frame: display-heading(display-empty w display-menu-title(screen w))
       bar: display-footer-h(w)
       top: add(display-header-h(w) 7)
       step: add(display-body-h(w) 4)
       frame: Match screen [
         (8 Let {frame: display-menu-list(frame screen w h selected flags)
                  frame: display-triggers(frame w sub(h add(bar 52)) model)}
              display-text(frame 8 sub(h add(bar 19)) 2 3 If eq(len(detail) 0) "Empty slot" display-compact-text(detail w)))
         (9 Let {frame: display-text(frame 8 top 1 0 display-target-label(display-get(model 15 255)))
                  frame: display-text(frame 8 add(top step) 1 0 If display-bit(flags 1) If display-wide(w) "Left connected" "Left online" "Left offline")
                  frame: display-text(frame 8 add(top mul(step 2)) 1 0 If display-bit(flags 2) If display-wide(w) "Right connected" "Right online" "Right offline")
                  button-y: add(top mul(step 3))
                  frame: display-selection(frame 1 button-y sub(w 2) step wide)}
              display-text(frame 8 add(button-y 2) 1 0 If wide "UF2 Bootloader" "UF2 loader"))
         (11 Let {count: display-get(model 14 0) captured: display-get(model 13 0)
                  frame: display-text(frame 8 top 1 0
                    str-cat(to-string(min(count add(captured 1))) " of " to-string(count)))}
               display-triggers(frame w add(top step) model))
         (12 If eq(len(detail) 0) frame
               Let {end: sub(h add(bar 17))}
                 display-line-text(frame w div(end 2) end div(mul(end 3) 4)
                   detail display-text-width(detail w 0)))
         (_ display-menu-list(frame screen w h selected flags))]
       notice: display-notice(display-get(model 11 0))
       frame: If eq(len(notice) 0) frame
         display-text(display-box(frame 1 0 sub(h mul(2 bar)) w bar 0 0) 8 sub(h mul(2 bar)) 4 3 notice)}
    If eq(screen 17) frame display-bar(frame w h screen)

Def display-touch-help Fn {w: s64 h: s64}
  Let {frame: display-heading(display-empty w "Touch Help")
       top: add(display-header-h(w) 8)
       step: add(display-body-h(w) 4)
       frame: display-text(frame 8 top 1 0 "Swipe to move")
       frame: display-text(frame 8 add(top step) 1 0 "Tap to select")}
    display-text(frame 8 add(top mul(step 2)) 1 0 "Hold to go back")

Def display-host-help Fn {w: s64 h: s64}
  Let {frame: display-heading(display-empty w "Host Target")
       top: add(display-header-h(w) 6)
       step: add(display-body-h(w) 2)
       frame: display-text(frame 7 top 1 0 "Braille menu")
       frame: display-text(frame 7 add(top step) 1 0 "T for Target")
       frame: display-text(frame 7 add(top mul(step 2)) 1 0 "Up / Down")
       frame: display-text(frame 7 add(top mul(step 3)) 1 0 "Return picks")}
    display-bar(frame w h 13)

Def display-help-rows The s64 4
Def display-help-entry-count The s64 10
Def display-host-help-index
  mul(div(add(display-help-entry-count sub(display-help-rows 1)) display-help-rows) display-help-rows)

Def display-help Fn {w: s64 h: s64 selected: s64}
  Let {frame: display-heading(display-empty w "Help")
       start: mul(div(min(selected sub(display-help-entry-count 1)) display-help-rows) display-help-rows)
       count: min(display-help-rows sub(display-help-entry-count start))
       top: add(display-header-h(w) 4)
       row-h: div(sub(h add(top mul(2 display-footer-h(w)))) display-help-rows)
       frame: fold(Fn {acc: display-frame i: s64}
         Let {index: add(start i)
              entry: Match index [
                (8 display-entry-row(If display-wide(w) "Escape goes back" "Esc Back" [259]))
                (9 display-entry-row(If display-wide(w) "Return opens" "Ret opens" [273]))
                (_ display-navigation-entry(index))]}
           Match entry [({label: label cells: cells mnemonic: mnemonic}
             Let {y: add(top mul(i row-h))
                  dot-w: If display-wide(w) 18 12
                  acc: display-text(acc 7 y 1 0 label)}
               display-dots(acc sub(w add(dot-w 9)) y dot-w sub(row-h 2) get(cells 0)))]
         frame range(0 count))}
    display-bar(frame w h 13)

Export {iface: "kb:firmware/display@0.1.0"
        name: render
        params: {width: u16 height: u16 model: list(u8) detail: string}
        result: tuple(list(command) list(string))}
Def render Fn {width: u16 height: u16 model: list(u8) detail: string}
  Let {w: display-width(width) h: display-height(height)
       screen: display-get(model 0 0)
       selected: to-s64(display-get-wide(model 1))
       frame: Match screen [
         (0 display-home(w h model))
         (1 display-typing(w h model detail))
         (4 display-grid(w h model selected))
         (5 display-detail(w h model selected))
         (13 If If ge(selected display-host-help-index) display-bit(display-get(model 7 0) 32) false
               display-host-help(w h)
               display-help(w h selected))
         (14 display-bootloader(w h model selected))
         (18 display-touch-help(w h))
         (_ display-menu(screen w h model selected detail))]}
    Match frame [({commands: commands labels: labels} tuple2(commands labels))]

Def display-result Fn {screen: u8 selected: u16 action: u8}
  to-u32(add(add(to-s64(screen) mul(to-s64(selected) 256)) mul(to-s64(action) 16777216)))

Def display-command Fn {cell: u16}
  Match cell [
    (257 1)
    (264 2)
    (260 3)
    (288 4)
    (263 5)
    (312 6)
    (316 7)
    (313 8)
    (259 9)
    (273 10)
    (128 10)
    (320 10)
    (384 10)
    (_ 0)]

Def display-menu-shortcut Fn {cell: u16}
  Match rem(to-s64(cell) 256) [
    (23 3)
    (9 7)
    (3 8)
    (25 9)
    (30 15)
    (_ 0)]

Def display-menu-open Fn {screen: u8 selected: u16}
  Match screen [
    (2 display-result(get(The list(u8) [3 7 8 9 15] min(selected 4)) 0 0))
    (3 display-result(4 to-u16(mul(min(to-s64(selected) sub(display-section-count 1)) 256)) 0))
    (6 display-result(get(The list(u8) [3 7 8 9 15] min(selected 4)) 0 0))
    (7 display-result(screen selected 1))
    (8 display-result(10 selected 1))
    (9 display-result(14 0 0))
    (10 display-result(screen selected 1))
    (12 display-result(screen selected 1))
    (14 display-result(screen selected 1))
    (15 display-result(16 0 1))
    (17 display-result(get(The list(u8) [3 9 18] min(selected 2)) 0 0))
    (_ display-result(screen selected 0))]

Def display-menu-back Fn {screen: u8 selected: u16}
  Match screen [
    (2 display-result(1 0 2))
    (3 display-result(6 0 2))
    (6 display-result(1 0 2))
    (7 display-result(6 1 2))
    (8 display-result(6 2 2))
    (9 display-result(6 3 2))
    (10 display-result(8 selected 2))
    (11 display-result(10 0 2))
    (12 display-result(8 0 2))
    (13 display-result(6 0 2))
    (14 display-result(9 0 2))
    (15 display-result(6 4 2))
    (17 display-result(1 0 2))
    (18 display-result(17 2 2))
    (_ display-result(6 0 2))]

Def display-menu-move Fn {screen: u8 selected: u16 command: s64 page: s64 host-switch: bool}
  Let {last: max(0 sub(display-menu-count(screen host-switch) 1))
       index: Match command [
         (1 sub(to-s64(selected) 1)) (2 add(to-s64(selected) 1))
         (3 sub(to-s64(selected) 1)) (4 add(to-s64(selected) 1))
         (5 0) (6 last)
         (7 sub(to-s64(selected) page)) (8 add(to-s64(selected) page))
         (_ to-s64(selected))]}
    display-result(screen to-u16(min(last max(0 index))) 0)

Def display-letter-index Fn {cell: u16 fallback: s64}
  Match cell [
    (1 0) (3 1) (9 2) (25 3) (17 4) (11 5) (27 6) (19 7)
    (10 8) (26 9) (5 10) (7 11) (13 12) (29 13) (21 14) (15 15)
    (31 16) (23 17) (14 18) (30 19) (37 20) (39 21) (58 22) (45 23)
    (61 24) (53 25) (_ fallback)]

Def display-reference-move Fn {screen: u8 selected: u16 command: s64 cell: u16 w: s64 h: s64}
  Let {section: min(div(to-s64(selected) 256) sub(display-section-count 1)) index: rem(to-s64(selected) 256)
       last: sub(display-section-length(section) 1)
       cols: If eq(screen 5) 1 display-reference-cols(section display-wide(w))
       page: If eq(screen 5) 1 display-reference-page(section w h)
       moved: Match command [
         (1 sub(index 1)) (2 add(index 1))
         (3 sub(index cols)) (4 add(index cols))
         (5 0) (6 last)
         (7 sub(index page)) (8 add(index page))
         (_ If lt(section 2) display-letter-index(cell index) index)]}
    Match command [
      (9 If eq(screen 5) display-result(4 selected 0) display-result(3 to-u16(section) 0))
      (10 display-result(5 selected 0))
      (_ If eq(cell 29) display-result(screen to-u16(mul(256 rem(add(section 1) display-section-count))) 0)
         If eq(cell 15) display-result(screen to-u16(mul(256 rem(add(section sub(display-section-count 1)) display-section-count))) 0)
         display-result(screen to-u16(add(mul(section 256) max(0 min(last moved)))) 0))]

Def display-help-move Fn {selected: u16 command: s64 cell: u16 host-switch: bool}
  Let {last-entry: sub(display-help-entry-count 1)
       last: If host-switch display-host-help-index last-entry
       current: to-s64(selected)
       page: display-help-rows
       next: If eq(cell 29) add(current page)
             If eq(cell 15) sub(current page)
             Match command [
               (1 sub(current 1)) (2 add(current 1))
               (3 sub(current 1)) (4 add(current 1))
               (5 0) (6 last)
               (7 sub(current page)) (8 add(current page))
               (_ current)]
       next: If If gt(next last-entry) lt(next display-host-help-index) false
         If gt(next current) display-host-help-index last-entry
         next}
    If eq(command 9) display-menu-back(13 selected)
      display-result(13 to-u16(min(last max(0 next))) 0)

Export {iface: "kb:firmware/display@0.1.0"
        name: navigate
        params: {screen: u8 selected: u16 cell: u16 view: viewport}
        result: u32}
Def navigate Fn {screen: u8 selected: u16 cell: u16 view: viewport}
  If eq(cell 0) display-result(screen selected 0)
    display-navigate(screen selected display-command(cell) cell view)

DefType navigation [up down left right enter escape]

Export {iface: "kb:firmware/display@0.1.0"
        name: navigate-action
        params: {screen: u8 selected: u16 action: navigation view: viewport}
        result: u32}
Def navigate-action Fn {screen: u8 selected: u16 action: navigation view: viewport}
  If If lt(screen 2) true eq(screen 16)
    display-result(2 0 6)
    Let {command: Match action [(up 3) (down 4) (left 1) (right 2) (enter 10) (escape 9)]}
      display-navigate(screen selected command 0 view)

Def display-navigate Fn {screen: u8 selected: u16 command: s64 cell: u16 view: viewport}
  Match view [({width: width height: height host-switch: host-switch}
  Let {w: display-width(width) h: display-height(height)
       shortcut: display-menu-shortcut(cell)
       editor: If eq(screen 11) true eq(screen 12)}
    If lt(screen 2)
      If eq(cell 287) display-result(2 0 6) display-result(screen selected 0)
    If eq(screen 13) display-help-move(selected command cell host-switch)
    If If editor eq(cell 275) eq(cell 19)
      display-result(13 0 4)
    If eq(cell 287)
      If eq(screen 2) display-result(6 0 0) display-result(2 0 6)
    If eq(screen 4) display-reference-move(screen selected command cell w h)
    If eq(screen 5) display-reference-move(screen selected command cell w h)
    If eq(command 9) display-menu-back(screen selected)
    If editor
      If If eq(command 10) If eq(screen 12) true eq(cell 0) false
        display-menu-open(screen selected)
        display-result(screen selected If eq(cell 0) 0 3)
    If If If eq(shortcut 15) host-switch gt(shortcut 0) If eq(screen 2) true eq(screen 6) false
      display-result(shortcut 0 0)
    If eq(command 10) display-menu-open(screen selected)
      display-menu-move(screen selected command display-menu-rows(screen w h host-switch) host-switch))]
