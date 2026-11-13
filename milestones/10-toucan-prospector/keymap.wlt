Package "kb:toucan-keymap@0.1.0"

DefType key-state [pressed released]
DefType key-event {position: u16 state: key-state timestamp-ms: u32}
DefType key {usage: u16 modifiers: u8}
DefType action [press(key) release(key) tap(key) release-all]
DefType input {events: list(key-event) now-ms: u32}
DefType output {state: list(u8) events: list(key-event) actions: list(action) wake-at-ms: option(u32)}

Def base-usage Fn {position: u16}
  to-u8(get([43 20 26 8 21 23 28 24 12 18 19 49
             41 4 22 7 9 10 11 13 14 15 51 52
             227 29 27 6 25 5 17 16 54 55 56 229
             225 224 44 40 42 0] position))

Def layer-usage Fn {position: u16}
  to-u8(get([43 30 31 32 33 34 35 36 37 38 39 49
             41 58 59 60 61 62 63 64 65 66 67 68
             227 49 45 46 47 48 80 81 82 79 56 229
             225 224 44 40 42 0] position))

Def initial-state Fn {state: list(u8)}
  If eq(len(state) 42) state map(Fn {n: s64} The u8 0 range(0 42))

Def transition Fn {acc: output event: key-event}
  Match acc [({state: state events: events actions: actions wake-at-ms: wake}
    Match event [({position: position state: edge timestamp-ms: time}
      If ge(position 42)
        {state: state events: push(events event) actions: actions wake-at-ms: wake}
        If eq(position 41)
          {state: put(state 41 to-u8(If eq(edge pressed) 1 0)) events: events actions: actions wake-at-ms: wake}
          If eq(edge pressed)
            If eq(get(state position) 0)
              Let {usage: If eq(get(state 41) 0) base-usage(position) layer-usage(position)}
                {state: put(state position usage) events: events actions: push(actions press({usage: to-u16(usage) modifiers: The u8 0})) wake-at-ms: wake}
              acc
            Let {usage: get(state position)}
              If eq(usage 0) acc
                {state: put(state position The u8 0) events: events actions: push(actions release({usage: to-u16(usage) modifiers: The u8 0})) wake-at-ms: wake})])]

Export {iface: "kb:firmware/keyboard@0.1.0" name: process params: {state: list(u8) input: input} result: output}
Def process Fn {state: list(u8) input: input}
  Match input [({events: events now-ms: now}
    fold(transition
      {state:      initial-state(state)
       events:     The list(key-event) []
       actions:    The list(action) []
       wake-at-ms: none}
      events))]

Export {iface:  "kb:firmware/keyboard@0.1.0"
        name:   transform-output
        params: {state:   list(u8)
                 actions: list(action)}
        result: tuple(list(u8) list(action))}
Def transform-output Fn {state: list(u8) actions: list(action)}
  tuple2(state actions)
