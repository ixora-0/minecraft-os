use spin::Mutex;

pub static PS2_MOUSE: Mutex<Ps2Mouse> = Mutex::new(Ps2Mouse::default());

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MouseButtons {
    None,
    Left,
    Right,
    Middle,
    LeftRight,
    LeftMiddle,
    RightMiddle,
    All,
}

impl MouseButtons {
    pub fn from_state_byte(b: u8) -> Self {
        match b & 0x07 {
            0b000 => MouseButtons::None,
            0b001 => MouseButtons::Left,
            0b010 => MouseButtons::Right,
            0b011 => MouseButtons::LeftRight,
            0b100 => MouseButtons::Middle,
            0b101 => MouseButtons::LeftMiddle,
            0b110 => MouseButtons::RightMiddle,
            0b111 => MouseButtons::All,
            _ => MouseButtons::None,
        }
    }
}
impl MouseButtons {
    pub fn is_left_down(&self) -> bool {
        matches!(
            self,
            MouseButtons::Left | MouseButtons::LeftMiddle | MouseButtons::LeftRight
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MousePacket {
    pub buttons: MouseButtons,
    pub dx: i16,
    pub dy: i16,
    pub sign_x: bool,
    pub sign_y: bool,
    pub byte_4: Option<u8>,
}

impl MousePacket {
    pub fn from_bytes(byte_state: u8, byte_x: u8, byte_y: u8, byte_4: Option<u8>) -> Self {
        // not handling overflow
        // const X_OVERFLOW_BIT: u8 = 1 << 6;
        // const Y_OVERFLOW_BIT: u8 = 1 << 7;
        // let x_overflow = (byte_state & X_OVERFLOW_BIT) != 0;
        // let y_overflow = (byte_state & Y_OVERFLOW_BIT) != 0;

        const SIGN_X_BIT: u8 = 1 << 4;
        const SIGN_Y_BIT: u8 = 1 << 5;
        let sign_x = (byte_state & SIGN_X_BIT) != 0;
        let sign_y = (byte_state & SIGN_Y_BIT) != 0;
        let dx = byte_x as i16 - ((sign_x as i16) << 8);
        let dy = byte_y as i16 - ((sign_y as i16) << 8);
        let dy = -dy; // make it so that down is positive
        let buttons = MouseButtons::from_state_byte(byte_state);

        MousePacket {
            buttons,
            dx,
            dy,
            sign_x,
            sign_y,
            byte_4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MouseType {
    Standard = 0x00,
    Intellimouse = 0x03,
    Explorer = 0x04,
    Unknown = 0xFF,
}

impl MouseType {
    pub fn from_type_id(type_id: u8) -> Self {
        match type_id {
            0x00 => MouseType::Standard,
            0x03 => MouseType::Intellimouse,
            0x04 => MouseType::Explorer,
            _ => MouseType::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct Ps2Mouse {
    pub x: i16,
    pub y: i16,
    pub buttons: MouseButtons,
    /// Number of packet received (0-3)
    pub packet_state: u8,
    pub packet_bytes: [u8; 4],
    pub mouse_type: MouseType,
}
impl const Default for Ps2Mouse {
    fn default() -> Self {
        Ps2Mouse {
            x: 0,
            y: 0,
            buttons: MouseButtons::None,
            packet_state: 0,
            packet_bytes: [0; 4],
            mouse_type: MouseType::Unknown,
        }
    }
}

impl Ps2Mouse {
    pub fn set_mouse_type(&mut self, type_id: u8) {
        self.mouse_type = MouseType::from_type_id(type_id);
    }

    pub fn handle_interrupt(&mut self, data: u8) -> Option<MousePacket> {
        let bytes_needed = match self.mouse_type {
            MouseType::Standard => 3,
            MouseType::Explorer => 4,
            MouseType::Intellimouse => 4,
            MouseType::Unknown => return None,
        };

        self.packet_bytes[self.packet_state as usize] = data;
        self.packet_state += 1;

        if self.packet_state < bytes_needed {
            return None;
        }

        self.packet_state = 0;

        let byte_state = self.packet_bytes[0];
        let byte_x = self.packet_bytes[1];
        let byte_y = self.packet_bytes[2];
        let byte_4 = if bytes_needed == 4 {
            Some(self.packet_bytes[3])
        } else {
            None
        };

        let packet = MousePacket::from_bytes(byte_state, byte_x, byte_y, byte_4);
        self.x += packet.dx;
        self.y += packet.dy;
        self.buttons = packet.buttons;

        log::trace!(
            "Mouse: x={}, y={}, buttons={:?}",
            self.x,
            self.y,
            self.buttons
        );
        return Some(packet);
    }

    pub fn reset_position(&mut self) {
        self.x = 0;
        self.y = 0;
    }
}
