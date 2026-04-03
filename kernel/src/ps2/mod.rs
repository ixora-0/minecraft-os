use core::fmt::Debug;
use x86_64::instructions::port::Port;

pub mod keyboard;
pub mod mouse;

use crate::ps2::keyboard::KeyboardType;

pub use self::keyboard::{PS2_KEYBOARD, ScancodeSet, with_ps2_keyboard, with_ps2_keyboard_mut};
pub use self::mouse::{MouseClicks, PS2_MOUSE, with_ps2_mouse, with_ps2_mouse_mut};

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

    pub fn expect_ack(&mut self) -> bool {
        self.read_data() == 0xFA
    }

    pub fn read_with_timeout(&mut self) -> Option<u8> {
        if self.wait_output_with_timeout() {
            Some(self.read_data())
        } else {
            None
        }
    }

    fn send_keyboard_command(&mut self, cmd: KeyboardCommand) {
        self.write_data(cmd as u8);
    }

    fn send_mouse_command(&mut self, cmd: MouseCommand) {
        self.send_command(Ps2Command::SendToMouse);
        self.write_data(cmd as u8);
    }

    fn log_ack(&mut self, context: &str) -> bool {
        let ack = self.read_data();
        match ack {
            0xFA => {
                log::trace!("PS/2 {} ack passed", context);
                true
            }
            _ => {
                log::warn!("PS/2 {} ack failed: {:#X} (expected 0xFA)", context, ack);
                false
            }
        }
    }

    fn log_port_test(&mut self, port: u8, result: u8) {
        match result {
            0x00 => log::trace!("PS/2 port {} test passed", port),
            0x01 => log::warn!("PS/2 port {} test failed: clock stuck low", port),
            0x02 => log::warn!("PS/2 port {} test failed: clock stuck high", port),
            0x03 => log::warn!("PS/2 port {} test failed: data stuck low", port),
            0x04 => log::warn!("PS/2 port {} test failed: data stuck high", port),
            _ => log::warn!("PS/2 port {} test failed: {:#04X}", port, result),
        }
    }
}

fn init_keyboard(ps2: &mut Ps2Controller) {
    // reset
    {
        ps2.write_data(KeyboardCommand::Reset as u8);
        let b1 = ps2.read_with_timeout();
        if b1.is_none() {
            log::warn!("PS/2 port 1: no device connected");
            return;
        }
        let b1 = b1.unwrap();
        let b2 = ps2.read_data();
        let valid = (b1 == 0xFA && b2 == 0xAA) || (b1 == 0xAA && b2 == 0xFA);
        if !valid {
            log::warn!("PS/2 unexpected reset response: {:#X}, {:#X}", b1, b2);
            return;
        }
    }

    // identify
    {
        ps2.send_keyboard_command(KeyboardCommand::DisableScanning);
        ps2.log_ack("disable scanning");
        ps2.send_keyboard_command(KeyboardCommand::Identify);
        ps2.log_ack("identify");
        let b1 = ps2.read_with_timeout();
        let b2 = b1.and_then(|_| ps2.read_with_timeout());
        match KeyboardType::from_identify_bytes(b1, b2) {
            KeyboardType::Unknown(_, _) | KeyboardType::Invalid => {
                log::warn!("PS/2 keyboard: Can't identify keyboard type")
            }
            t => log::info!("PS/2 keyboard: {:?} type detected", t),
        }
    }

    // set scancode set
    {
        ps2.send_keyboard_command(KeyboardCommand::Scancode);
        ps2.log_ack("set scancode set command");
        ps2.write_data(0x02);
        ps2.log_ack("set scancode set 2");
    }
    // get scancode est
    {
        ps2.send_keyboard_command(KeyboardCommand::Scancode);
        ps2.log_ack("get scancode set command");
        ps2.write_data(0x00);
        ps2.log_ack("get scancode set");
        let scancode_set_id = ps2.read_data();
        match scancode_set_id {
            0x43 | 0x01 => ScancodeSet::Set1,
            0x41 | 0x02 => ScancodeSet::Set2,
            0x3F | 0x03 => ScancodeSet::Set3,
            id => {
                log::warn!("PS/2 keyboard: Unknown scancode set: {:#X}", id);
                ScancodeSet::Unknown
            }
        };
    }
    // we will re enable translation, and so we'd be always reading set 1 regardless
    *PS2_KEYBOARD.lock() = keyboard::Ps2Keyboard::default();

    // enable scanning
    ps2.send_keyboard_command(KeyboardCommand::EnableScanning);
    ps2.log_ack("enable scanning");
}

fn init_mouse(ps2: &mut Ps2Controller) {
    // reset
    ps2.send_mouse_command(MouseCommand::Reset);
    match ps2.read_with_timeout() {
        Some(0xFA) => {}
        Some(0xFE) => log::warn!("PS/2 mouse resend request"),
        Some(ack) => log::warn!("PS/2 mouse reset ack failed: {:#X}", ack),
        None => {
            log::warn!("PS/2 port 2: no device connected");
            return;
        }
    }
    let reset_result = match ps2.read_with_timeout() {
        Some(r) => r,
        None => return,
    };
    match reset_result {
        0xAA => log::trace!("PS/2 mouse self test passed"),
        0xFC => {
            log::warn!("PS/2 mouse self test failed");
            return;
        }
        _ => {
            log::warn!("PS/2 mouse reset result: {:#X}", reset_result);
            return;
        }
    }
    // Read device ID (mouse sends 0x00 after self-test result)
    if let Some(device_id) = ps2.read_with_timeout() {
        PS2_MOUSE.lock().set_mouse_type(device_id);
    }

    ps2.send_mouse_command(MouseCommand::SetDefaults);
    ps2.log_ack("mouse set defaults");

    // identify
    ps2.send_mouse_command(MouseCommand::ReadDeviceType);
    ps2.log_ack("mouse identify");
    if let Some(mouse_id) = ps2.read_with_timeout() {
        match mouse::MouseType::from_type_id(mouse_id) {
            mouse::MouseType::Unknown => {
                log::warn!("PS/2 momuse: Can't identify mouse type")
            }
            t => log::info!("PS/2 mouse: {:?} type detected", t),
        }
        PS2_MOUSE.lock().set_mouse_type(mouse_id);
    }

    // enable reporting
    ps2.send_mouse_command(MouseCommand::EnableReporting);
    ps2.log_ack("mouse enable reporting");
}

pub fn init() {
    let mut ps2 = Ps2Controller::new();
    // https://wiki.osdev.org/I8042_PS/2_Controller

    // step 2: check for ps/2 controller
    if !crate::acpi::has_ps2_controller() {
        log::warn!("FADT says there're no PS/2 controller");
    }

    // step 3: disable devices
    ps2.send_command(Ps2Command::DisableFirstPort);
    ps2.send_command(Ps2Command::DisableSecondPort);

    // step 4: flush the output buffer
    ps2.flush_buffer();

    // step 5: set the controller configuration byte
    let mut config = Ps2Config::from_byte(ps2.read_config());
    if !config.validate() {
        log::warn!("PS/2 config is not well defined");
    }
    config.set_irq1(false);
    config.set_translation(false);
    config.set_first_port_clock(true);
    ps2.write_config(config.to_byte());

    // step 6: perform controller self test
    {
        ps2.send_command(Ps2Command::SelfTest);
        let self_test_res = ps2.read_data();
        if self_test_res != 0x55 {
            log::warn!(
                "PS/2 self test failed: {:#X} (expected 0x55)",
                self_test_res
            );
        }
        ps2.write_config(config.to_byte());
    }

    // step 7: determine if there are 2 channels
    ps2.send_command(Ps2Command::EnableSecondPort);
    // clock should be enabled since we just sent the command to enable the second port
    let is_dual_channel = config.irq12_enabled();
    log::info!("PS/2 dual channel: {}", is_dual_channel);
    ps2.send_command(Ps2Command::DisableSecondPort);
    if is_dual_channel {
        config.set_irq12(false);
        config.set_second_port_clock(true);
        ps2.write_config(config.to_byte());
    }

    // step 8: perform interface tests
    ps2.send_command(Ps2Command::TestFirstPort);
    let port1_test = ps2.read_data();
    let port1_works = port1_test == 0x00;
    ps2.log_port_test(1, port1_test);
    let port2_works = if is_dual_channel {
        ps2.send_command(Ps2Command::TestSecondPort);
        let port2_test = ps2.read_data();
        ps2.log_port_test(2, port2_test);
        port2_test == 0x00
    } else {
        false
    };

    // step 9: enable devices
    if port1_works {
        ps2.send_command(Ps2Command::EnableFirstPort);
        config.set_irq1(true);
    }
    if port2_works {
        ps2.send_command(Ps2Command::EnableSecondPort);
        config.set_irq12(true);
        config.set_second_port_clock(true);
    }
    ps2.write_config(config.to_byte());

    if port1_works {
        init_keyboard(&mut ps2);
    }
    if port2_works {
        init_mouse(&mut ps2);
    }

    // re enable translation
    config.set_translation(true);
    ps2.write_config(config.to_byte());

    log::trace!(
        "PS/2 final config: {:?}",
        Ps2Config::from_byte(ps2.read_config())
    );
}
