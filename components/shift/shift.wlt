Package "kb:shift@0.1.0"

DefType key-state [pressed released]
DefType key-event {position: u16 state: key-state timestamp-ms: u32}
DefType key {usage: u16 modifiers: u8}
DefType action [press(key) release(key) tap(key) release-all]
DefType input {events: list(key-event) now-ms: u32}
DefType output {state: list(u8) events: list(key-event) actions: list(action) wake-at-ms: option(u32)}

Def shift-key Fn {key: key}
  Match key [({usage: usage modifiers: modifiers}
    {usage: usage modifiers:
      If lt(usage 4) modifiers
        If gt(usage 29) modifiers
          If eq(rem(div(modifiers 2) 2) 1) modifiers to-u8(add(modifiers 2))})]

Def shift-action Fn {action: action}
  Match action [
    ((press key) press(shift-key(key)))
    ((release key) release(shift-key(key)))
    ((tap key) tap(shift-key(key)))
    (release-all release-all)]

Export {iface: "kb:firmware/keyboard@0.1.0" name: process params: {state: list(u8) input: input} result: output}
Def process Fn {state: list(u8) input: input}
  Match input [({events: events now-ms: now} {state: state events: events actions: The list(action) [] wake-at-ms: none})]

Export {iface: "kb:firmware/keyboard@0.1.0" name: transform-output params: {state: list(u8) actions: list(action)} result: tuple(list(u8) list(action))}
Def transform-output Fn {state: list(u8) actions: list(action)}
  tuple2(state map(shift-action actions))
