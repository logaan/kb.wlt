Package "kb:toucan-keymap@0.1.0"

DefType key-state [pressed released]
DefType key-event {position: u16 state: key-state timestamp-ms: u32}
DefType key {usage: u16 modifiers: u8}
DefType action [press(key) release(key) tap(key) release-all]
DefType input {events: list(key-event) now-ms: u32}
DefType output {state:      list(u8)
                events:     list(key-event)
                actions:    list(action)
                wake-at-ms: option(u32)}

Def base-usage Fn {position: u16}
  to-u8(get([43 20 26 8 21 23 28 24 12 18 19 41
             224 4 22 7 9 10 11 13 14 15 51 52
             225 29 27 6 25 5 17 16 54 55 56 45
             0 42 227 44 40 225] position))

Def layer-usage Fn {position: u16}
  to-u8(get([0 43 80 41 82 0 0 43 81 79 0 0
             0 0 0 0 0 0 0 0 0 0 0 0
             0 225 224 226 42 0 0 227 40 44 0 0
             0 42 227 0 40 0] position))

Def initial-state Fn {state: list(u8)}
  If eq(len(state) 42) state map(Fn {n: s64} The u8 0 range(0 42))

Def transition Fn {acc: output event: key-event}
  Match acc [({state: state events: events actions: actions wake-at-ms: wake}
    Match event [({position: position state: edge timestamp-ms: time}
      If ge(position 42)
        {state:      state
         events:     push(events event)
         actions:    actions
         wake-at-ms: wake}
        If eq(position 36)
          {state:       put(state 36 to-u8(If eq(edge pressed) 1 0))
           events:      events
           actions:     actions
           wake-at-ms:  wake}
          If eq(edge pressed)
            If eq(get(state position) 0)
              Let {usage: If eq(get(state 36) 0)
                            base-usage(position)
                            layer-usage(position)}
                If eq(usage 0) acc
                  {state:      put(state position usage)
                   events:     events
                   actions:    push(actions press({usage: to-u16(usage)
                                                   modifiers: The u8 0}))
                   wake-at-ms: wake}
              acc
            Let {usage: get(state position)}
              If eq(usage 0) acc
                {state:      put(state position The u8 0)
                 events:     events
                 actions:    push(actions release({usage: to-u16(usage)
                                                   modifiers: The u8 0}))
                 wake-at-ms: wake})])]

Export {iface:  "kb:firmware/keyboard@0.1.0"
        name:   process
        params: {state: list(u8)
        input:  input}
        result: output}
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
