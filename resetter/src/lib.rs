#![no_std]

pub const PULSE_MS: u32 = 100;
pub const BOOT_GAP_MS: u32 = 200;
pub const MAX_LINE_LEN: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    One,
    Two,
}

impl Target {
    pub const fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Reset,
    Boot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Ping,
    Status,
    Pulse {
        target: Target,
        operation: Operation,
    },
}

impl Command {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ping => "PING",
            Self::Status => "STATUS",
            Self::Pulse {
                target: Target::One,
                operation: Operation::Reset,
            } => "RESET1",
            Self::Pulse {
                target: Target::Two,
                operation: Operation::Reset,
            } => "RESET2",
            Self::Pulse {
                target: Target::One,
                operation: Operation::Boot,
            } => "BOOT1",
            Self::Pulse {
                target: Target::Two,
                operation: Operation::Boot,
            } => "BOOT2",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    LineTooLong,
    InvalidByte,
}

pub fn parse(line: &[u8]) -> Result<Command, ParseError> {
    let start = line
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(line.len());
    let end = line
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    let line = &line[start..end];
    for command in [
        Command::Ping,
        Command::Status,
        Command::Pulse {
            target: Target::One,
            operation: Operation::Reset,
        },
        Command::Pulse {
            target: Target::Two,
            operation: Operation::Reset,
        },
        Command::Pulse {
            target: Target::One,
            operation: Operation::Boot,
        },
        Command::Pulse {
            target: Target::Two,
            operation: Operation::Boot,
        },
    ] {
        if line.eq_ignore_ascii_case(command.name().as_bytes()) {
            return Ok(command);
        }
    }
    Err(ParseError::UnknownCommand)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineParser {
    bytes: [u8; MAX_LINE_LEN],
    len: usize,
    error: Option<ParseError>,
    after_cr: bool,
}

impl Default for LineParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LineParser {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_LINE_LEN],
            len: 0,
            error: None,
            after_cr: false,
        }
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Command, ParseError>> {
        if byte == b'\n' && self.after_cr {
            self.after_cr = false;
            return None;
        }
        self.after_cr = byte == b'\r';
        if byte == b'\r' || byte == b'\n' {
            let result = if let Some(error) = self.error.take() {
                Some(Err(error))
            } else if self.bytes[..self.len].iter().all(u8::is_ascii_whitespace) {
                None
            } else {
                Some(parse(&self.bytes[..self.len]))
            };
            self.len = 0;
            return result;
        }
        if self.error.is_some() {
            return None;
        }
        if !byte.is_ascii_graphic() && byte != b' ' && byte != b'\t' {
            self.error = Some(ParseError::InvalidByte);
        } else if self.len == MAX_LINE_LEN {
            self.error = Some(ParseError::LineTooLong);
        } else {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    FirstPulse,
    Gap,
    SecondPulse,
}

impl Phase {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::FirstPulse => "LOW1",
            Self::Gap => "GAP",
            Self::SecondPulse => "LOW2",
        }
    }

    pub const fn pulls_low(self) -> bool {
        matches!(self, Self::FirstPulse | Self::SecondPulse)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Busy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pong,
    Status([Phase; 2]),
    Accepted(Command),
    Complete(Command),
    Busy(Command),
    Error(ParseError),
    QueueFull,
}

impl Response {
    pub fn encode(self, destination: &mut [u8]) -> Result<usize, core::fmt::Error> {
        use core::fmt::Write;
        let mut output = TextBuffer {
            destination,
            len: 0,
        };
        match self {
            Self::Pong => write!(output, "PONG kb-resetter 1\r\n")?,
            Self::Status(phases) => write!(
                output,
                "STATUS 1={} 2={}\r\n",
                phases[0].name(),
                phases[1].name()
            )?,
            Self::Accepted(command) => write!(output, "ACK {}\r\n", command.name())?,
            Self::Complete(command) => write!(output, "DONE {}\r\n", command.name())?,
            Self::Busy(command) => write!(output, "ERR BUSY {}\r\n", command.name())?,
            Self::Error(ParseError::UnknownCommand) => write!(output, "ERR UNKNOWN_COMMAND\r\n")?,
            Self::Error(ParseError::LineTooLong) => write!(output, "ERR LINE_TOO_LONG\r\n")?,
            Self::Error(ParseError::InvalidByte) => write!(output, "ERR INVALID_BYTE\r\n")?,
            Self::QueueFull => write!(output, "ERR QUEUE_FULL\r\n")?,
        }
        Ok(output.len)
    }
}

struct TextBuffer<'a> {
    destination: &'a mut [u8],
    len: usize,
}

impl core::fmt::Write for TextBuffer<'_> {
    fn write_str(&mut self, value: &str) -> core::fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(core::fmt::Error)?;
        let space = self
            .destination
            .get_mut(self.len..end)
            .ok_or(core::fmt::Error)?;
        space.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Channel {
    phase: Phase,
    operation: Operation,
    changed_at: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    channels: [Channel; 2],
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Controller {
    pub const fn new() -> Self {
        Self {
            channels: [Channel {
                phase: Phase::Idle,
                operation: Operation::Reset,
                changed_at: 0,
            }; 2],
        }
    }

    pub const fn phases(&self) -> [Phase; 2] {
        [self.channels[0].phase, self.channels[1].phase]
    }

    pub fn start(
        mut self,
        target: Target,
        operation: Operation,
        now_ms: u32,
    ) -> Result<Self, Busy> {
        let channel = &mut self.channels[target.index()];
        if channel.phase != Phase::Idle {
            return Err(Busy);
        }
        *channel = Channel {
            phase: Phase::FirstPulse,
            operation,
            changed_at: now_ms,
        };
        Ok(self)
    }

    pub fn advance(mut self, now_ms: u32) -> (Self, [Option<Command>; 2]) {
        let mut completed = [None; 2];
        for (index, channel) in self.channels.iter_mut().enumerate() {
            let elapsed = now_ms.wrapping_sub(channel.changed_at);
            let next = match channel.phase {
                Phase::FirstPulse
                    if elapsed >= PULSE_MS && channel.operation == Operation::Boot =>
                {
                    Phase::Gap
                }
                Phase::FirstPulse | Phase::SecondPulse if elapsed >= PULSE_MS => Phase::Idle,
                Phase::Gap if elapsed >= BOOT_GAP_MS => Phase::SecondPulse,
                _ => continue,
            };
            channel.phase = next;
            channel.changed_at = now_ms;
            if next == Phase::Idle {
                completed[index] = Some(Command::Pulse {
                    target: if index == 0 { Target::One } else { Target::Two },
                    operation: channel.operation,
                });
            }
        }
        (self, completed)
    }
}
