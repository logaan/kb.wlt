Package "kb:led-dim@0.1.0"

Import {pkg: "kb:firmware/led@0.1.0" as: next}

DefType state {phase: u8 speed: u8 last-change-ms: u32 button-was-down: bool}
DefType output {state: state red: u8 green: u8 blue: u8}

Export {iface: "kb:firmware/led@0.1.0" name: step params: {state: state now-ms: u32 button-down: bool} result: output}
Def step Fn {state: state now-ms: u32 button-down: bool}
  Match next/step(state now-ms button-down) [
    ({state: next-state red: red green: green blue: blue}
      {state: next-state red: to-u8(div(red 4)) green: to-u8(div(green 4)) blue: to-u8(div(blue 4))})]
