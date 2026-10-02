//! OS-neutral platform boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatingSystem {
    Linux,
    Windows,
    MacOS,
    Other,
}
pub fn operating_system() -> OperatingSystem {
    if cfg!(target_os = "linux") {
        OperatingSystem::Linux
    } else if cfg!(target_os = "windows") {
        OperatingSystem::Windows
    } else if cfg!(target_os = "macos") {
        OperatingSystem::MacOS
    } else {
        OperatingSystem::Other
    }
}
pub fn platform_name() -> &'static str {
    match operating_system() {
        OperatingSystem::Linux => "linux",
        OperatingSystem::Windows => "windows",
        OperatingSystem::MacOS => "macos",
        OperatingSystem::Other => "other",
    }
}
