use kb_resetter::{
    Command, Controller, LineParser, Operation, ParseError, Phase, Response, Target, parse,
};

fn pulse(target: Target, operation: Operation) -> Command {
    Command::Pulse { target, operation }
}

#[test]
fn commands_accept_case_whitespace_and_reject_ambiguous_inputs() {
    for (line, expected) in [
        ("ping", Command::Ping),
        (" STATUS \t", Command::Status),
        ("Reset1", pulse(Target::One, Operation::Reset)),
        ("reset2", pulse(Target::Two, Operation::Reset)),
        ("BOOT1", pulse(Target::One, Operation::Boot)),
        ("boot2", pulse(Target::Two, Operation::Boot)),
    ] {
        assert_eq!(parse(line.as_bytes()), Ok(expected));
    }
    for line in [
        "",
        "RESET",
        "RESET3",
        "BOOT1 BOOT2",
        "RESET1NOW",
        "STATUS 1",
    ] {
        assert_eq!(parse(line.as_bytes()), Err(ParseError::UnknownCommand));
    }
}

#[test]
fn serial_packets_can_split_commands_and_contain_multiple_lines() {
    let mut parser = LineParser::new();
    let mut parsed = Vec::new();
    for packet in [b"BO".as_slice(), b"OT1\r", b"\nPING\nRE", b"SET2\r\n\n"] {
        for byte in packet {
            if let Some(command) = parser.push(*byte) {
                parsed.push(command);
            }
        }
    }
    assert_eq!(
        parsed,
        vec![
            Ok(pulse(Target::One, Operation::Boot)),
            Ok(Command::Ping),
            Ok(pulse(Target::Two, Operation::Reset))
        ]
    );
}

#[test]
fn invalid_and_oversized_lines_are_discarded_until_a_terminator() {
    let mut parser = LineParser::new();
    for byte in [b'x'; 80] {
        assert_eq!(parser.push(byte), None);
    }
    assert_eq!(parser.push(b'\n'), Some(Err(ParseError::LineTooLong)));
    for byte in b"PI\0NG" {
        assert_eq!(parser.push(*byte), None);
    }
    assert_eq!(parser.push(b'\n'), Some(Err(ParseError::InvalidByte)));
    for byte in b"PING" {
        assert_eq!(parser.push(*byte), None);
    }
    assert_eq!(parser.push(b'\n'), Some(Ok(Command::Ping)));
}

#[test]
fn reset_pulls_low_for_a_hundred_milliseconds_then_releases() {
    let controller = Controller::new();
    assert_eq!(controller.phases(), [Phase::Idle; 2]);
    let controller = controller.start(Target::One, Operation::Reset, 20).unwrap();
    assert!(controller.phases()[0].pulls_low());
    let (controller, completed) = controller.advance(119);
    assert_eq!(completed, [None; 2]);
    assert!(controller.phases()[0].pulls_low());
    let (controller, completed) = controller.advance(120);
    assert_eq!(controller.phases(), [Phase::Idle; 2]);
    assert_eq!(
        completed,
        [Some(pulse(Target::One, Operation::Reset)), None]
    );
}

#[test]
fn boot_is_two_ground_pulses_with_a_released_gap() {
    let controller = Controller::new()
        .start(Target::Two, Operation::Boot, 0)
        .unwrap();
    let (controller, completed) = controller.advance(100);
    assert_eq!(controller.phases(), [Phase::Idle, Phase::Gap]);
    assert!(!controller.phases()[1].pulls_low());
    assert_eq!(completed, [None; 2]);
    let (controller, _) = controller.advance(299);
    assert_eq!(controller.phases()[1], Phase::Gap);
    let (controller, _) = controller.advance(300);
    assert_eq!(controller.phases()[1], Phase::SecondPulse);
    let (controller, completed) = controller.advance(400);
    assert_eq!(controller.phases(), [Phase::Idle; 2]);
    assert_eq!(completed, [None, Some(pulse(Target::Two, Operation::Boot))]);
}

#[test]
fn two_targets_run_independently_and_same_target_rejects_overlap() {
    let controller = Controller::new()
        .start(Target::One, Operation::Boot, 0)
        .unwrap();
    assert!(controller.start(Target::One, Operation::Reset, 10).is_err());
    let controller = controller.start(Target::Two, Operation::Reset, 50).unwrap();
    let (controller, _) = controller.advance(100);
    assert_eq!(controller.phases(), [Phase::Gap, Phase::FirstPulse]);
    let (controller, completed) = controller.advance(150);
    assert_eq!(completed[1], Some(pulse(Target::Two, Operation::Reset)));
    assert_eq!(controller.phases(), [Phase::Gap, Phase::Idle]);
    assert!(controller.start(Target::Two, Operation::Boot, 151).is_ok());
}

#[test]
fn delayed_scheduling_keeps_minimum_pulse_and_gap_durations() {
    let controller = Controller::new()
        .start(Target::One, Operation::Boot, 0)
        .unwrap();
    let (controller, _) = controller.advance(120);
    let (controller, _) = controller.advance(319);
    assert_eq!(controller.phases()[0], Phase::Gap);
    let (controller, _) = controller.advance(350);
    let (controller, _) = controller.advance(449);
    assert_eq!(controller.phases()[0], Phase::SecondPulse);
    let (controller, completed) = controller.advance(450);
    assert_eq!(controller.phases()[0], Phase::Idle);
    assert!(completed[0].is_some());
}

#[test]
fn clock_wrap_does_not_shorten_a_reset() {
    let controller = Controller::new()
        .start(Target::One, Operation::Reset, u32::MAX - 49)
        .unwrap();
    let (controller, completed) = controller.advance(49);
    assert_eq!(completed, [None; 2]);
    let (controller, completed) = controller.advance(50);
    assert_eq!(controller.phases()[0], Phase::Idle);
    assert!(completed[0].is_some());
}

#[test]
fn all_responses_fit_one_usb_packet_with_line_termination() {
    for response in [
        Response::Pong,
        Response::Status([Phase::SecondPulse, Phase::FirstPulse]),
        Response::Accepted(pulse(Target::One, Operation::Reset)),
        Response::Complete(pulse(Target::Two, Operation::Boot)),
        Response::Busy(pulse(Target::Two, Operation::Reset)),
        Response::Error(ParseError::UnknownCommand),
        Response::Error(ParseError::InvalidByte),
        Response::Error(ParseError::LineTooLong),
        Response::QueueFull,
    ] {
        let mut buffer = [0; 64];
        let length = response.encode(&mut buffer).unwrap();
        assert!(length < 64);
        assert!(buffer[..length].ends_with(b"\r\n"));
        assert!(response.encode(&mut [0; 2]).is_err());
    }
}
