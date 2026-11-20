use kb_client_session::{
    BOOTLOADER_LEFT, BOOTLOADER_PROSPECTOR, BOOTLOADER_RIGHT, Change, Links, Session,
};
use kb_component_runtime::DisplayGuest;
use kb_core::{
    host_target::{Target, TargetConnection},
    matrix_link::{BrailleSwitch, Side, braille_keys, joined_matrix},
    transport::{InputGate, SessionGate, can_enter_local_bootloader},
};
use kb_display::Scene;

const fn key(row: u8, column: u8) -> u32 {
    1 << Side::Left.matrix_index(row, column).unwrap()
}

const BRAILLE: u32 = key(3, 3);
const SPACE: u32 = key(3, 5);
const DOT_1: u32 = key(1, 4);
const DOT_2: u32 = key(1, 3);
const DOT_3: u32 = key(1, 2);
const DOT_4: u32 = key(2, 4);
const DOT_5: u32 = key(2, 3);
const DOT_6: u32 = key(2, 2);
const DOT_7: u32 = key(1, 1);
const LEADER: u32 = SPACE | DOT_1 | DOT_2 | DOT_3 | DOT_4 | DOT_5;
const OPEN: u32 = SPACE | DOT_7;
const BACK: u32 = SPACE | DOT_1 | DOT_2;
const NEXT: u32 = SPACE | DOT_4;
const PREVIOUS: u32 = SPACE | DOT_1;

const NO_HOST: Links = Links {
    online: 1,
    battery: [255; 2],
    target: Target::Prospector,
    connection: TargetConnection::Connecting,
};

struct LeftHalf {
    app: Session,
    input: InputGate,
    output: SessionGate,
    mode: BrailleSwitch,
    braille: bool,
    now: u32,
}

impl LeftHalf {
    fn new() -> Self {
        let mut app = Session::new(Default::default());
        app.enable_host_switch();
        app.set_viewport(144, 168);
        app.set_bootloader_devices(BOOTLOADER_LEFT);
        Self {
            app,
            input: InputGate::new(),
            output: SessionGate::new(),
            mode: BrailleSwitch::default(),
            braille: false,
            now: 0,
        }
    }

    fn scan(&mut self, left: u32, guest: &mut impl DisplayGuest) -> Change {
        assert!(self.output.token().is_none());
        self.now += 10;
        let keys = joined_matrix(left, 0);
        let sample = self
            .input
            .observe(self.output.generation(), true, keys, false);
        if sample.cancel {
            self.app.cancel_for_link_change(braille_keys(keys));
        }
        let next = self.mode.update(self.now, sample.keys);
        assert!(next || !self.braille);
        if next != self.braille {
            self.app.cancel(0);
            self.output = self.output.invalidate();
            self.input.acknowledge_generation(self.output.generation());
        }
        self.braille = next;
        let change = if self.braille && sample.keys.is_some() {
            self.app
                .update(braille_keys(keys), self.now, guest)
                .unwrap()
        } else {
            Change::default()
        };
        assert!(change.strokes.is_empty() && change.target.is_none() && change.bluetooth.is_none());
        if change.reset {
            self.output = self.output.invalidate();
            self.input.acknowledge_generation(self.output.generation());
        }
        change
    }

    fn chord(&mut self, keys: u32, guest: &mut impl DisplayGuest) -> Change {
        assert_eq!(self.scan(keys, guest).bootloader, 0);
        if keys & SPACE != 0 {
            let change = self.scan(SPACE, guest);
            assert_eq!(self.scan(0, guest).bootloader, 0);
            change
        } else {
            self.scan(0, guest)
        }
    }

    fn press(&mut self, keys: u32, guest: &mut impl DisplayGuest) {
        assert_eq!(self.chord(keys, guest).bootloader, 0);
    }

    fn screen(&self) -> (u8, u16) {
        let model = self.app.model(NO_HOST);
        let bytes = model.bytes.as_slice();
        (bytes[0], u16::from_le_bytes([bytes[1], bytes[2]]))
    }

    fn render(&self, guest: &mut impl DisplayGuest) -> Scene {
        let model = self.app.model(NO_HOST);
        let data = guest
            .render(144, 168, model.bytes.as_slice(), model.detail.as_str())
            .unwrap();
        Scene::from_bytes(data.as_slice()).unwrap()
    }
}

fn has_command(scene: &Scene, matches: impl Fn(&[u8]) -> bool) -> bool {
    let mut commands = scene.as_bytes();
    while let Some(opcode) = commands.first() {
        let length = match opcode {
            1 | 2 | 5 => 11,
            4 | 6 => 12,
            3 => 8 + usize::from(commands[7]),
            _ => unreachable!(),
        };
        if matches(&commands[..length]) {
            return true;
        }
        commands = &commands[length..];
    }
    false
}

fn has_label(scene: &Scene, label: &[u8]) -> bool {
    has_command(scene, |command| command[0] == 3 && &command[8..] == label)
}

pub(super) fn run(guest: &mut impl DisplayGuest) {
    let mut left = LeftHalf::new();
    left.scan(0, guest);
    left.scan(BRAILLE, guest);
    assert!(!left.braille);
    left.scan(0, guest);
    assert!(left.braille);

    left.press(LEADER, guest);
    assert_eq!(left.screen(), (2, 0));
    left.press(DOT_1 | DOT_2 | DOT_3 | DOT_5, guest);
    assert_eq!(left.screen(), (3, 0));
    left.press(OPEN, guest);
    assert_eq!(left.screen(), (4, 0));
    assert!(has_label(&left.render(guest), b"a"));
    left.press(SPACE | DOT_1 | DOT_4 | DOT_5 | DOT_6, guest);
    assert_eq!(left.screen(), (4, 12));
    assert!(has_label(&left.render(guest), b"m"));
    left.press(DOT_1 | DOT_3 | DOT_5 | DOT_6, guest);
    assert_eq!(left.screen(), (4, 25));
    left.press(OPEN, guest);
    assert_eq!(left.screen(), (5, 25));
    let detail = left.render(guest);
    assert!(has_label(&detail, b"z"));
    assert!(has_command(&detail, |command| command[0] == 4 && command[9] == 57));
    for expected in [(4, 25), (3, 0), (6, 0), (1, 0)] {
        left.press(BACK, guest);
        assert_eq!(left.screen(), expected);
    }

    left.press(LEADER, guest);
    left.press(DOT_1 | DOT_4 | DOT_5, guest);
    assert_eq!(left.screen(), (9, 0));
    left.press(OPEN, guest);
    assert_eq!(left.screen(), (14, 0));
    assert_eq!(
        left.app.model(NO_HOST).bytes.as_slice()[15..17],
        [BOOTLOADER_LEFT, 0]
    );
    assert!(has_label(&left.render(guest), b"Left half"));
    for (index, device) in [(1, BOOTLOADER_RIGHT), (2, BOOTLOADER_PROSPECTOR)] {
        left.press(NEXT, guest);
        assert_eq!(left.screen(), (14, index));
        left.press(OPEN, guest);
        let model = left.app.model(NO_HOST);
        assert_eq!(model.bytes.as_slice()[15] & device, 0);
        assert_eq!(model.bytes.as_slice()[16], 0);
    }
    left.press(PREVIOUS, guest);
    left.press(PREVIOUS, guest);
    left.press(OPEN, guest);
    for _ in 0..3 {
        left.press(NEXT, guest);
    }
    assert_eq!(left.screen(), (14, 3));
    let request = left.chord(OPEN, guest).bootloader;
    assert_eq!(request, BOOTLOADER_LEFT);
    assert_eq!(request & (BOOTLOADER_RIGHT | BOOTLOADER_PROSPECTOR), 0);
    assert!(can_enter_local_bootloader(request, 0, BOOTLOADER_LEFT));
    assert!(!can_enter_local_bootloader(
        request | BOOTLOADER_RIGHT,
        0,
        BOOTLOADER_LEFT
    ));
    assert!(left.output.token().is_none());
}
