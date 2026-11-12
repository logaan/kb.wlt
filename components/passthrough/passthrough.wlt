Package "kb:passthrough@0.1.0"

DefType key-state [pressed released]
DefType key-event {position: u16 state: key-state timestamp-ms: u32}
DefType key {usage: u16 modifiers: u8}
DefType action [press(key) release(key) tap(key) release-all]
DefType input {events: list(key-event) now-ms: u32}
DefType output {state: list(u8) events: list(key-event) actions: list(action) wake-at-ms: option(u32)}

Export {iface: "kb:firmware/keyboard@0.1.0" name: process params: {state: list(u8) input: input} result: output}
Def process Fn {state: list(u8) input: input}
  Match input [({events: events now-ms: now} {state: state events: events actions: The list(action) [] wake-at-ms: none})]

Export {iface: "kb:firmware/keyboard@0.1.0" name: transform-output params: {state: list(u8) actions: list(action)} result: tuple(list(u8) list(action))}
Def transform-output Fn {state: list(u8) actions: list(action)}
  tuple2(state actions)
