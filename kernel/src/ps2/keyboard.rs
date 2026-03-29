use spin::Mutex;

pub use pc_keyboard::{KeyCode, KeyState};

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

#[derive(Debug, Clone, Copy)]
pub struct KeyStates(u128);

impl KeyStates {
    /// set the state of a key (pressed or released)
    pub fn update_state(&mut self, key_code: u8, down: bool) {
        let bit = 1 << key_code;
        if down {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }

    pub fn is_pressed(&self, key: KeyCode) -> bool {
        let bit = 1u128 << (key as u8);
        self.0 & bit != 0
    }
}
impl const Default for KeyStates {
    fn default() -> Self {
        Self(0)
    }
}

// not used by Ps2Keyboard right now
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScancodeSet {
    Set1,
    Set2,
    Set3,
    Unknown,
}

pub struct Ps2Keyboard {
    keyboard: pc_keyboard::Keyboard<pc_keyboard::layouts::Us104Key, pc_keyboard::ScancodeSet1>,
    pub key_states: KeyStates,
}

impl Ps2Keyboard {
    pub fn handle_interrupt(&mut self, scancode: u8) {
        if let Ok(Some(key_event)) = self.keyboard.add_byte(scancode) {
            let key_code = key_event.code as u8;
            let key_down = key_event.state == pc_keyboard::KeyState::Down;
            let event_clone = key_event.clone();
            self.keyboard.process_keyevent(key_event);
            self.key_states.update_state(key_code, key_down);
            log::trace!("{:?}", event_clone);
        }
    }

    pub fn is_down(&self, key: KeyCode) -> bool {
        self.key_states.is_pressed(key)
    }
}

impl const Default for Ps2Keyboard {
    fn default() -> Self {
        Ps2Keyboard {
            keyboard: pc_keyboard::Keyboard::new(
                pc_keyboard::ScancodeSet1::new(),
                pc_keyboard::layouts::Us104Key,
                pc_keyboard::HandleControl::Ignore,
            ),
            key_states: KeyStates::default(),
        }
    }
}
