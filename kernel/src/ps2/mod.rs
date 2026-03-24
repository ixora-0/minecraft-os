use core::fmt::Debug;
use x86_64::instructions::port::Port;

pub mod keyboard;
pub mod mouse;

use crate::ps2::keyboard::KeyboardType;

pub use self::keyboard::{PS2_KEYBOARD, ScancodeSet};

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Ps2Command {
    DisableFirstPort = 0xAD,
    DisableSecondPort = 0xA7,
    ReadConfig = 0x20,
    WriteConfig = 0x60,
    SelfTest = 0xAA,
    EnableSecondPort = 0xA8,
    TestFirstPort = 0xAB,
    TestSecondPort = 0xA9,
    EnableFirstPort = 0xAE,
    SendToMouse = 0xD4,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum KeyboardCommand {
    Scancode = 0xF0,
    Identify = 0xF2,
    EnableScanning = 0xF4,
    DisableScanning = 0xF5,
    Reset = 0xFF,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum MouseCommand {
    Reset = 0xFF,
    DisableReporting = 0xF5,
    EnableReporting = 0xF4,
    SetDefaults = 0xF6,
    SetRemoteMode = 0xF0,
    SetStreamMode = 0xEA,
    ReadDeviceType = 0xF2,
}

#[derive(Clone, Copy)]
pub struct Ps2Config(u8);

macro_rules! bit_field {
    (
        $(#[$getter_doc:meta])* => $getter_name:ident,
        $(#[$setter_doc:meta])* => $setter_name:ident,
        $bit:expr
    ) => {
        $(#[$getter_doc])*
        pub fn $getter_name(self) -> bool {
            self.0 & $bit != 0
        }
        $(#[$setter_doc])*
        pub fn $setter_name(&mut self, enabled: bool) {
            if enabled {
                self.0 |= $bit;
            } else {
                self.0 &= !$bit;
            }
        }
    };
}

macro_rules! bit_field_inverted {
    (
        $(#[$getter_doc:meta])* => $getter_name:ident,
        $(#[$setter_doc:meta])* => $setter_name:ident,
        $bit:expr
    ) => {
        $(#[$getter_doc])*
        pub fn $getter_name(self) -> bool {
            self.0 & $bit == 0
        }
        $(#[$setter_doc])*
        pub fn $setter_name(&mut self, enabled: bool) {
            if enabled {
                self.0 &= !$bit;
            } else {
                self.0 |= $bit;
            }
        }
    };
}

macro_rules! bit_field_readonly {
    (
        $(#[$getter_doc:meta])* => $getter_name:ident,
        $bit:expr
    ) => {
        $(#[$getter_doc])*
        pub fn $getter_name(self) -> bool {
            self.0 & $bit != 0
        }
    };
}

impl Ps2Config {
    pub fn from_byte(byte: u8) -> Self {
        Self(byte)
    }

    pub fn to_byte(self) -> u8 {
        self.0
    }

    bit_field! {
        /// Returns true if the first PS/2 port (keyboard) interrupt is enabled.
        => irq1_enabled,
        /// Enables or disables the first PS/2 port (keyboard) interrupt.
        => set_irq1,
        0b00000001
    }
    bit_field! {
        /// Returns true if the second PS/2 port (mouse) interrupt is enabled. Only if 2 PS/2 ports are supported.
        => irq12_enabled,
        /// Enables or disables the second PS/2 port (mouse) interrupt. Only if 2 PS/2 ports are supported.
        => set_irq12,
        0b00000010
    }
    bit_field_readonly! {
        /// Returns 1 if the system passed POST, 0 otherwise.
        => system_flag,
        0b00000100
    }
    bit_field_readonly! {
        /// Returns true if reserved bit 3 is set (should be 0).
        => reserved_bit3,
        0b00001000
    }
    bit_field_inverted! {
        /// Returns true if the first PS/2 port (keyboard) clock is enabled.
        => first_port_clock_enabled,
        /// Enables or disables the first PS/2 port (keyboard) clock.
        => set_first_port_clock,
        0b00010000
    }
    bit_field_inverted! {
        /// Returns true if the second PS/2 port (mouse) clock is enabled. Only if 2 PS/2 ports are supported.
        => second_port_clock_enabled,
        /// Enables or disables the second PS/2 port (mouse) clock. Only if 2 PS/2 ports are supported.
        => set_second_port_clock,
        0b00100000
    }
    bit_field! {
        /// Returns true if first PS/2 port (keyboard) translation is enabled.
        => translation_enabled,
        /// Enables or disables first PS/2 port (keyboard) translation.
        => set_translation,
        0b01000000
    }
    bit_field_readonly! {
        /// Returns true if reserved bit 7 is set (should be 0).
        => reserved_bit7,
        0b10000000
    }

    pub fn validate(&self) -> bool {
        let mut ok = true;
        if !self.system_flag() {
            log::warn!("PS/2 controller configuration byte says OS shouldn't be running?");
            ok = false;
        };
        if self.reserved_bit3() {
            log::warn!("PS/2 controller configuration byte has reserved bit 3 set");
            ok = false;
        }
        if self.reserved_bit7() {
            log::warn!("PS/2 controller configuration byte has reserved bit 7 set");
            ok = false;
        }
        ok
    }
}

impl Debug for Ps2Config {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#010b}", self.0)
    }
}

pub struct Ps2Controller {
    cmd: Port<u8>,
    data: Port<u8>,
}

impl Ps2Controller {
    pub fn new() -> Self {
        Self {
            cmd: Port::new(0x64),
            data: Port::new(0x60),
        }
    }

    fn wait_input_empty(&mut self) {
        unsafe { while self.cmd.read() & 0b10 != 0 {} }
    }

    fn wait_output_full(&mut self) {
        unsafe { while self.cmd.read() & 0b01 == 0 {} }
    }

    /// returns false if timeout
    fn wait_output_with_timeout(&mut self) -> bool {
        unsafe {
            for _ in 0..100_000 {
                if self.cmd.read() & 0b01 != 0 {
                    return true;
                }
            }
        }
        false
    }

    pub fn send_command(&mut self, cmd: Ps2Command) {
        self.wait_input_empty();
        unsafe {
            self.cmd.write(cmd as u8);
        }
    }

    pub fn read_data(&mut self) -> u8 {
        self.wait_output_full();
        unsafe { self.data.read() }
    }

    pub fn write_data(&mut self, data: u8) {
        self.wait_input_empty();
        unsafe {
            self.data.write(data);
        }
    }

    pub fn read_config(&mut self) -> u8 {
        self.send_command(Ps2Command::ReadConfig);
        self.read_data()
    }

    pub fn write_config(&mut self, config: u8) {
        self.send_command(Ps2Command::WriteConfig);
        self.write_data(config);
    }

    pub fn flush_buffer(&mut self) {
        unsafe {
            while self.cmd.read() & 0b01 != 0 {
                let _ = self.data.read();
            }
        }
    }
}

pub fn init() {
    let mut ps2 = Ps2Controller::new();
    // https://wiki.osdev.org/I8042_PS/2_Controller

    log::trace!("PS/2 init step 2/10: Checking for PS/2 controller");
    if !crate::acpi::has_ps2_controller() {
        log::warn!("FADT says there're no PS/2 controller");
    }

    // step 3: disable devices
    log::trace!("PS/2 init step 3/10: Disabling PS/2 devices");
    ps2.send_command(Ps2Command::DisableFirstPort);
    ps2.send_command(Ps2Command::DisableSecondPort);

    // step 4: flush the output buffer
    log::trace!("PS/2 init step 4/10: Flushing output buffer");
    ps2.flush_buffer();

    // step 5: set the controller configuration byte
    log::trace!("PS/2 init step 5/10: Setting controller configuration byte");
    let mut config = Ps2Config::from_byte(ps2.read_config());
    log::trace!("PS/2 initial config: {:?}", config);
    if !config.validate() {
        log::warn!(
            "PS/2 config is not well defined. This might mean PS/2 controller would not working properly."
        );
    }
    config.set_irq1(false);
    config.set_translation(false);
    config.set_first_port_clock(true);
    ps2.write_config(config.to_byte());

    // step 6: perform controller self test
    log::trace!("PS/2 init step 6/10: Performing controller self test");
    ps2.send_command(Ps2Command::SelfTest);
    let self_test = ps2.read_data();
    match self_test {
        0x55 => log::trace!("PS/2 self test passed"),
        _ => log::warn!("PS/2 self test failed: {:#X} (expected 0x55)", self_test),
    }
    ps2.write_config(config.to_byte());

    // step 7: determine if there are 2 channels
    log::trace!("PS/2 init step 7/10: Checking for dual channel");
    ps2.send_command(Ps2Command::EnableSecondPort);
    let mut config = Ps2Config::from_byte(ps2.read_config());
    // clock should be enabled since we just sent the command to enable the second port
    let is_dual_channel = config.irq12_enabled();
    log::info!("PS/2 dual channel: {}", is_dual_channel);
    if is_dual_channel {
        ps2.send_command(Ps2Command::DisableSecondPort);
        config.set_irq12(false);
        config.set_second_port_clock(true);
        ps2.write_config(config.to_byte());
    }

    // step 8: perform interface tests
    log::trace!("PS/2 init step 8/10: Performing interface tests");
    ps2.send_command(Ps2Command::TestFirstPort);
    let port1_test = ps2.read_data();
    let port1_works = port1_test == 0x00;
    match port1_test {
        0x00 => log::trace!("PS/2 port 1 test passed"),
        0x01 => log::warn!("PS/2 port 1 test failed: 0x01 (clock line stuck low) (expected 0x00)"),
        0x02 => log::warn!("PS/2 port 1 test failed: 0x02 (clock line stuck high) (expected 0x00)"),
        0x03 => log::warn!("PS/2 port 1 test failed: 0x03 (data line stuck low) (expected 0x00)"),
        0x04 => log::warn!("PS/2 port 1 test failed: 0x04 (data line stuck high) (expected 0x00)"),
        _ => log::warn!(
            "PS/2 port 1 test failed: {:#04X} (unknown response) (expected 0x00)",
            port1_test
        ),
    }
    let mut port2_works = false;
    if is_dual_channel {
        ps2.send_command(Ps2Command::TestSecondPort);
        let port2_test = ps2.read_data();
        port2_works = port2_test == 0x00;
        match port2_test {
            0x00 => log::trace!("PS/2 port 2 test passed"),
            0x01 => {
                log::warn!("PS/2 port 2 test failed: 0x01 (clock line stuck low) (expected 0x00)")
            }
            0x02 => {
                log::warn!("PS/2 port 2 test failed: 0x02 (clock line stuck high) (expected 0x00)")
            }
            0x03 => {
                log::warn!("PS/2 port 2 test failed: 0x03 (data line stuck low) (expected 0x00)")
            }
            0x04 => {
                log::warn!("PS/2 port 2 test failed: 0x04 (data line stuck high) (expected 0x00)")
            }
            _ => log::warn!(
                "PS/2 port 2 test failed: {:#04X} (unknown response) (expected 0x00)",
                port2_test
            ),
        }
    }

    // flush output buffer before enabling devices to avoid spurious interrupts
    ps2.flush_buffer();

    // step 9: enable devices
    log::trace!("PS/2 init step 9/10: Enabling devices");
    let mut config = Ps2Config::from_byte(ps2.read_config());
    if port1_works {
        ps2.send_command(Ps2Command::EnableFirstPort);
        config.set_irq1(true);
        log::trace!("PS/2 port 1 enabled with IRQ1");
    }
    if port2_works {
        ps2.send_command(Ps2Command::EnableSecondPort);
        config.set_irq12(true);
        config.set_second_port_clock(true);
        log::trace!("PS/2 port 2 enabled with IRQ12");
    }
    ps2.write_config(config.to_byte());

    // Step 10: Reset Device
    log::trace!("PS/2 init step 10/10: Resetting device");
    ps2.write_data(KeyboardCommand::Reset as u8);
    let byte1 = if ps2.wait_output_with_timeout() {
        ps2.read_data()
    } else {
        log::warn!("PS/2 port 1: no device connected (timeout)");
        log::info!(
            "PS/2 final config: {:?}",
            Ps2Config::from_byte(ps2.read_config())
        );
        return;
    };
    if byte1 == 0xFC {
        log::warn!("PS/2 port 1: device self test failed, ignoring device");
    } else {
        let byte2 = ps2.read_data();
        let valid = (byte1 == 0xFA && byte2 == 0xAA) || (byte1 == 0xAA && byte2 == 0xFA);
        if valid {
            log::trace!("PS/2 reset successful");
        } else {
            log::warn!(
                "PS/2 unexpected reset response: {:#X}, {:#X} (expected 0xFA and 0xAA)",
                byte1,
                byte2
            );
        }
    }

    // identify device
    log::trace!("PS/2: Identifying device");
    // first disable scanning
    ps2.write_data(KeyboardCommand::DisableScanning as u8);
    let ack = ps2.read_data();
    match ack {
        0xFA => log::trace!("PS/2 disable scanning ack passed"),
        _ => log::warn!(
            "PS/2 disable scanning ack failed: {:#X} (expected 0xFA)",
            ack
        ),
    }
    // send identify command
    ps2.write_data(KeyboardCommand::Identify as u8);
    let ack = ps2.read_data();
    match ack {
        0xFA => log::trace!("PS/2 identify ack passed"),
        _ => log::warn!("identify ack failed: {:#X} (expected 0xFA)", ack),
    }
    let b1 = if ps2.wait_output_with_timeout() {
        Some(ps2.read_data())
    } else {
        None
    };
    let b2 = if b1.is_some() {
        if ps2.wait_output_with_timeout() {
            Some(ps2.read_data())
        } else {
            None
        }
    } else {
        None
    };
    match KeyboardType::from_identify_bytes(b1, b2) {
        KeyboardType::Unknown(_, _) | KeyboardType::Invalid => {
            log::warn!("PS/2 keyboard: Can't identify keyboard type")
        }
        t => log::info!("PS/2 keyboard: {:?} type detected", t),
    }

    if port1_works {
        // enable scanning
        log::trace!("PS/2: Enabling scanning");
        ps2.write_data(KeyboardCommand::EnableScanning as u8);
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 scan enable ack passed"),
            _ => log::warn!("PS/2 scan enable ack failed: {:#X}", ack),
        }

        // set scancode set
        ps2.write_data(KeyboardCommand::Scancode as u8);
        ps2.write_data(0x02); // subcommand, set scancode set to set 2
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 keyboard set scancode set 2 success"),
            _ => log::warn!("PS/2 keyboard set scancode set 2 ack failed: {:#X}", ack),
        }

        // detect scancode set
        let scancode_set = {
            ps2.write_data(KeyboardCommand::Scancode as u8); // Get/set scan code set
            ps2.write_data(0x00); // subcommand, get current set
            let ack = ps2.read_data();
            match ack {
                0xFA => log::trace!("PS/2 keyboard get scancode set ack passed"),
                _ => log::warn!("PS/2 keyboard get scancode set ack failed: {:#X}", ack),
            }

            let scancode_set_id = loop {
                // some hardware gives additional ack, we ignore it
                let b = ps2.read_data();
                if b != 0xFA {
                    break b;
                }
            };
            log::debug!(
                "PS/2 keyboard: Detected scancode set ID: {:#X}",
                scancode_set_id
            );
            match scancode_set_id {
                0x43 | 0x01 => ScancodeSet::Set1,
                0x41 | 0x02 => ScancodeSet::Set2,
                0x3F | 0x03 => ScancodeSet::Set3,
                _ => ScancodeSet::Unknown,
            }
        };
        match scancode_set {
            keyboard::ScancodeSet::Set1 => log::info!("PS/2 keyboard: Using scancode set 1"),
            keyboard::ScancodeSet::Set2 => log::info!("PS/2 keyboard: Using scancode set 2"),
            keyboard::ScancodeSet::Set3 => log::info!("PS/2 keyboard: Using scancode set 3"),
            keyboard::ScancodeSet::Unknown => log::warn!("PS/2 keyboard: Unknown scancode set"),
        }

        // *keyboard::PS2_KEYBOARD.lock() = keyboard::Ps2Keyboard::new(scancode_set);
        *keyboard::PS2_KEYBOARD.lock() = keyboard::Ps2Keyboard::new(scancode_set);
    }

    if port2_works {
        log::trace!("PS/2: Initializing mouse");

        ps2.send_command(Ps2Command::SendToMouse);
        ps2.write_data(MouseCommand::Reset as u8);
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 mouse reset ack passed"),
            0xFE => log::warn!("PS/2 mouse resend request"),
            _ => log::warn!("PS/2 mouse reset ack failed: {:#X}", ack),
        }
        if !ps2.wait_output_with_timeout() {
            log::warn!("PS/2 mouse: no response after reset");
        } else {
            let reset_result = ps2.read_data();
            match reset_result {
                0xAA => log::trace!("PS/2 mouse self test passed"),
                0xFC => log::warn!("PS/2 mouse self test failed"),
                _ => log::warn!("PS/2 mouse reset result: {:#X}", reset_result),
            }
            // Read device ID (mouse sends 0x00 after self-test result)
            if ps2.wait_output_with_timeout() {
                let device_id = ps2.read_data();
                log::trace!("PS/2 mouse device ID from reset: {:#X}", device_id);
                self::mouse::PS2_MOUSE.lock().set_mouse_type(device_id);
            }
        }

        ps2.send_command(Ps2Command::SendToMouse);
        ps2.write_data(MouseCommand::SetDefaults as u8);
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 mouse set default ack passed"),
            _ => log::warn!("PS/2 mouse enable reporting ack failed: {:#X}", ack),
        }

        ps2.send_command(Ps2Command::SendToMouse);
        ps2.write_data(MouseCommand::ReadDeviceType as u8);
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 mouse identify ack passed"),
            _ => log::warn!("PS/2 mouse identify ack failed: {:#X}", ack),
        }
        if ps2.wait_output_with_timeout() {
            let mouse_id = ps2.read_data();
            match mouse_id {
                0x00 => log::info!("PS/2 mouse: Standard PS/2 mouse detected"),
                0x03 => log::info!("PS/2 mouse: Mouse with scrollwheel detected"),
                0x04 => log::info!("PS/2 mouse: 5-button mouse detected"),
                _ => log::warn!("PS/2 mouse: Unknown type ID: {:#X}", mouse_id),
            }
            self::mouse::PS2_MOUSE.lock().set_mouse_type(mouse_id);
        }

        ps2.send_command(Ps2Command::SendToMouse);
        ps2.write_data(MouseCommand::EnableReporting as u8);
        let ack = ps2.read_data();
        match ack {
            0xFA => log::trace!("PS/2 mouse enable reporting ack passed"),
            _ => log::warn!("PS/2 mouse enable reporting ack failed: {:#X}", ack),
        }
    }

    ps2.flush_buffer();

    log::info!(
        "PS/2 final config: {:?}",
        Ps2Config::from_byte(ps2.read_config())
    );
}
