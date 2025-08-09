use anyhow::Result;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

pub struct ServiceManager;

impl ServiceManager {
    pub fn install(server_url: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::install_service(server_url)
        }

        #[cfg(target_os = "linux")]
        {
            linux::install_service(server_url)
        }

        #[cfg(target_os = "macos")]
        {
            macos::install_service(server_url)
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(anyhow::anyhow!("Service installation not supported on this platform"))
        }
    }

    pub fn uninstall() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            windows::uninstall_service()
        }

        #[cfg(target_os = "linux")]
        {
            linux::uninstall_service()
        }

        #[cfg(target_os = "macos")]
        {
            macos::uninstall_service()
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(anyhow::anyhow!("Service management not supported on this platform"))
        }
    }

    pub fn status() -> Result<String> {
        #[cfg(target_os = "windows")]
        {
            windows::service_status()
        }

        #[cfg(target_os = "linux")]
        {
            linux::service_status()
        }

        #[cfg(target_os = "macos")]
        {
            macos::service_status()
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Ok("Not supported".to_string())
        }
    }
}
