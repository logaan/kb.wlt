Package "kb:led-demo@0.1.0"

DefType state {phase: u8 speed: u8 last-change-ms: u32 button-was-down: bool}
DefType output {state: state red: u8 green: u8 blue: u8}

Export {iface: "kb:firmware/led@0.1.0" name: step params: {state: state now-ms: u32 button-down: bool} result: output}
Def step Fn {state: state now-ms: u32 button-down: bool}
  Match state [
    ({phase: phase speed: speed last-change-ms: last button-was-down: was-down}
      Let {next-speed: If If button-down not(was-down) false to-u8(rem(add(speed 1) 3)) speed}
        Let {period: If eq(next-speed 0) 1000 If eq(next-speed 1) 400 100}
          Let {elapsed: If ge(now-ms last) sub(to-s64(now-ms) to-s64(last)) add(sub(4294967295 to-s64(last)) add(to-s64(now-ms) 1))}
            Let {advance: ge(elapsed period)}
              Let {next-phase: If advance to-u8(rem(add(phase 1) 6)) phase}
                {state: {phase: next-phase speed: next-speed last-change-ms: If advance now-ms last button-was-down: button-down}
                 red: to-u8(If If le(next-phase 1) true eq(next-phase 5) 255 0)
                 green: to-u8(If If ge(next-phase 1) le(next-phase 3) false 255 0)
                 blue: to-u8(If ge(next-phase 3) 255 0)})]
