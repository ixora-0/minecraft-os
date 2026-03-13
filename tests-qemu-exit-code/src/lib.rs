//! QEMU exit codes shared between the test runner and integration tests.
//!
//! This crate exists to avoid hardcoding exit codes in the test runner
//! (`src/bin/qemu-integration-test-runner.rs`). Both the runner and the
//! integration tests need this crate so that they use the same exit codes.
//!
//! Separate crate instead of defining these in `tests-integration`
//! because the test runner would need to depend on `tests-integration`,
//! which depends on `kernel`. Because `kernel` defines a `#[global_allocator]`,
//! the root package (where the test runner is) would then also use `kernel`'s
//! global allocator, which cause runtime errors. Having this shared crate avoids
//! this dependency chain.

#![no_std]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}
impl QemuExitCode {
    pub const TEST_SUCCEESS_EXIT_CODE: i32 = ((QemuExitCode::Success as i32) << 1) | 1;
}
