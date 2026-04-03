use heapless::Deque;
use spin::Mutex;
use x86_64::instructions::interrupts;

pub use pc_keyboard::{KeyCode, KeyState};

pub static PS2_KEYBOARD: Mutex<Ps2Keyboard> = Mutex::new(Ps2Keyboard::default());

pub fn with_ps2_keyboard<F, R>(f: F) -> R
where
    F: FnOnce(&Ps2Keyboard) -> R,
{
    interrupts::without_interrupts(|| {
        let keyboard = PS2_KEYBOARD.lock();
        f(&keyboard)
    })
}

/// Mutable keyboard access for cases that need to modify state (e.g., consuming events).
pub fn with_ps2_keyboard_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Ps2Keyboard) -> R,
{
    interrupts::without_interrupts(|| {
        let mut keyboard = PS2_KEYBOARD.lock();
        f(&mut keyboard)
    })
}

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

/// Keyboard event generated on key press, capturing key code and decoded character.
/// Use [`pop_event`] to retrieve events for input handling (e.g., console commands).
#[derive(Debug, Clone, Copy)]
pub struct KeyboardEvent {
    pub code: KeyCode,
    pub state: KeyState,
    pub character: Option<char>,
}

const KEYBOARD_EVENT_BUFFER: usize = 64;

pub struct Ps2Keyboard {
    keyboard: pc_keyboard::Keyboard<pc_keyboard::layouts::Us104Key, pc_keyboard::ScancodeSet1>,
    pub key_states: KeyStates,
    events: Deque<KeyboardEvent, KEYBOARD_EVENT_BUFFER>,
}

impl Ps2Keyboard {
    pub fn handle_interrupt(&mut self, scancode: u8) {
        if let Ok(Some(key_event)) = self.keyboard.add_byte(scancode) {
            let key_code = key_event.code as u8;
            let key_down = key_event.state == pc_keyboard::KeyState::Down;
            let decoded = self.keyboard.process_keyevent(key_event.clone());
            self.key_states.update_state(key_code, key_down);
            if key_down {
                let character = decoded.and_then(|decoded_key| match decoded_key {
                    pc_keyboard::DecodedKey::Unicode(ch) => Some(ch),
                    _ => None,
                });
                self.push_event(KeyboardEvent {
                    code: key_event.code,
                    state: key_event.state,
                    character,
                });
            }
            log::trace!("{:?}", key_event);
        }
    }

    pub fn is_down(&self, key: KeyCode) -> bool {
        self.key_states.is_pressed(key)
    }

    /// Retrieves a keyboard event from the interrupt-driven event buffer.
    /// Use this for character-based input (console) rather than checking KeyStates.
    pub fn pop_event(&mut self) -> Option<KeyboardEvent> {
        self.events.pop_front()
    }

    fn push_event(&mut self, event: KeyboardEvent) {
        if self.events.is_full() {
            let _ = self.events.pop_front();
        }
        self.events.push_back(event).ok();
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
            events: Deque::new(),
        }
    }
}
