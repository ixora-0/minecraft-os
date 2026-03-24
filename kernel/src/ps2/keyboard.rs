use spin::Mutex;

pub static PS2_KEYBOARD: Mutex<Ps2Keyboard> = Mutex::new(Ps2Keyboard::default());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardType {
    AT,
    MF2,
    /// ThinkPad / Spacesaver / compact
    ShortKeyboard,
    NcdN97,
    Keyboard122Key,
    JapaneseG,
    JapaneseP,
    JapaneseA,
    NcdSunLayout,
    Unknown(u8, u8),
    Invalid,
}
impl KeyboardType {
    pub fn from_identify_bytes(b1: Option<u8>, b2: Option<u8>) -> KeyboardType {
        match (b1, b2) {
            (None, None) => KeyboardType::AT,

            (Some(0xAB), Some(0x83)) | (Some(0xAB), Some(0x41)) | (Some(0xAB), Some(0xC1)) => {
                KeyboardType::MF2
            }

            (Some(0xAB), Some(0x84)) | (Some(0xAB), Some(0x54)) => KeyboardType::ShortKeyboard,

            (Some(0xAB), Some(0x85)) => KeyboardType::NcdN97,

            (Some(0xAB), Some(0x86)) => KeyboardType::Keyboard122Key,

            (Some(0xAB), Some(0x90)) => KeyboardType::JapaneseG,
            (Some(0xAB), Some(0x91)) => KeyboardType::JapaneseP,
            (Some(0xAB), Some(0x92)) => KeyboardType::JapaneseA,

            (Some(0xAC), Some(0xA1)) => KeyboardType::NcdSunLayout,

            (Some(x), Some(y)) => KeyboardType::Unknown(x, y),
            _ => KeyboardType::Invalid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScancodeSet {
    Set1,
    Set2,
    Set3,
    Unknown,
}

pub enum Ps2Keyboard {
    Set1(pc_keyboard::Keyboard<pc_keyboard::layouts::Us104Key, pc_keyboard::ScancodeSet1>),
    Set2(pc_keyboard::Keyboard<pc_keyboard::layouts::Us104Key, pc_keyboard::ScancodeSet2>),
}

impl Ps2Keyboard {
    pub const fn new(set: ScancodeSet) -> Self {
        match set {
            ScancodeSet::Set2 => Ps2Keyboard::Set2(pc_keyboard::Keyboard::new(
                pc_keyboard::ScancodeSet2::new(),
                pc_keyboard::layouts::Us104Key,
                pc_keyboard::HandleControl::Ignore,
            )),
            // defaults to set 1 if 3 or unknown
            _ => Ps2Keyboard::Set1(pc_keyboard::Keyboard::new(
                pc_keyboard::ScancodeSet1::new(),
                pc_keyboard::layouts::Us104Key,
                pc_keyboard::HandleControl::Ignore,
            )),
        }
    }

    pub fn handle_interrupt(&mut self, scancode: u8) {
        match self {
            Ps2Keyboard::Set1(keyboard) => {
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    keyboard.process_keyevent(key_event.clone());
                    log::trace!("{:?}, modifiers: {:?}", key_event, keyboard.get_modifiers());
                }
            }
            Ps2Keyboard::Set2(keyboard) => {
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    keyboard.process_keyevent(key_event.clone());
                    log::trace!("{:?}, modifiers: {:?}", key_event, keyboard.get_modifiers());
                }
            }
        }
    }
}

impl const Default for Ps2Keyboard {
    fn default() -> Self {
        Self::new(ScancodeSet::Set1)
    }
}
