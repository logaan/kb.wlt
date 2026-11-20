pub fn percentage(millivolts: u32) -> u8 {
    let curve = [
        (3300, 0),
        (3500, 5),
        (3650, 12),
        (3730, 25),
        (3800, 45),
        (3900, 65),
        (4000, 80),
        (4100, 92),
        (4200, 100),
    ];
    for pair in curve.windows(2) {
        let [(low, start), (high, end)] = [pair[0], pair[1]];
        if millivolts <= high {
            return (start + millivolts.saturating_sub(low) * (end - start) / (high - low)) as u8;
        }
    }
    100
}

pub fn encode(value: u8) -> u8 {
    if value <= 100 { value + 1 } else { 0 }
}

pub fn decode(value: u8) -> Option<u8> {
    match value {
        0 => Some(255),
        1..=101 => Some(value - 1),
        _ => None,
    }
}
