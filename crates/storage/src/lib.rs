use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Device not found: {0}")]
    NotFound(String),

    #[error("Permission denied reading device: {0}")]
    PermissionDenied(String),

    #[error("Invalid read range: offset {offset}, len {len}, size {size}")]
    OutOfBounds { offset: u64, len: usize, size: u64 },

    #[error("Platform unsupported: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Represents drive details returned when enumerating system volumes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DriveDetails {
    pub device_path: String,
    pub display_name: String,
    pub mount_point: Option<String>,
    pub size_bytes: u64,
    pub filesystem: Option<String>,
    pub is_read_only: bool,
    pub is_removable: bool,
}

/// Abstract read-only storage device interface.
pub trait StorageDevice: Send + Sync {
    /// Total size in bytes.
    fn size(&self) -> u64;

    /// Sector size in bytes (typically 512 or 4096).
    fn sector_size(&self) -> u32 {
        512
    }

    /// Read buffer at offset.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Convenient helper to read exact number of bytes.
    fn read_exact_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let read_bytes = self.read_at(offset, &mut buf)?;
        if read_bytes < len {
            buf.truncate(read_bytes);
        }
        Ok(buf)
    }
}

/// File-backed read-only storage device (e.g. disk images, fixtures).
pub struct FileStorageDevice {
    path: PathBuf,
    size: u64,
}

impl FileStorageDevice {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let metadata = file.metadata()?;
        let size = metadata.len();
        info!("Opened file storage device {:?} (size: {} bytes)", path, size);
        Ok(Self { path, size })
    }
}

impl StorageDevice for FileStorageDevice {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if offset >= self.size {
            return Ok(0);
        }
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(offset))?;
        let bytes_read = file.read(buf)?;
        Ok(bytes_read)
    }
}

/// Open a storage device from path (file or raw device string).
pub fn open_storage_device<P: AsRef<Path>>(path: P) -> Result<Box<dyn StorageDevice>> {
    let p = path.as_ref();

    #[cfg(target_os = "windows")]
    {
        let path_str = p.to_string_lossy();
        if path_str.starts_with(r"\\.\") || path_str.starts_with(r"\\?\") {
            let win_dev = WinStorageDevice::open(&path_str)?;
            return Ok(Box::new(win_dev));
        }
    }

    let file_dev = FileStorageDevice::open(p)?;
    Ok(Box::new(file_dev))
}

#[cfg(target_os = "windows")]
pub struct WinStorageDevice {
    handle: windows::Win32::Foundation::HANDLE,
    size: u64,
    sector_size: u32,
}

#[cfg(target_os = "windows")]
impl WinStorageDevice {
    pub fn open(device_path: &str) -> Result<Self> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, GetFileSizeEx, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows::Win32::System::IO::DeviceIoControl;
        use windows::Win32::System::Ioctl::IOCTL_DISK_GET_LENGTH_INFO;

        let wide_path: Vec<u16> = device_path.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .map_err(|e| {
            StorageError::PermissionDenied(format!(
                "Failed to open device {device_path}: {e} (Admin privileges required for raw disk access)"
            ))
        })?;

        if handle.is_invalid() || handle == INVALID_HANDLE_VALUE {
            return Err(StorageError::PermissionDenied(format!(
                "Failed to open device {device_path}: invalid handle (Admin privileges required for raw disk access)"
            )));
        }

        let mut length_info: i64 = 0;
        let mut bytes_returned: u32 = 0;

        let size = unsafe {
            let ioctl_res = DeviceIoControl(
                handle,
                IOCTL_DISK_GET_LENGTH_INFO,
                None,
                0,
                Some(&mut length_info as *mut i64 as *mut _),
                std::mem::size_of::<i64>() as u32,
                Some(&mut bytes_returned),
                None,
            );

            if ioctl_res.is_ok() && length_info > 0 {
                length_info as u64
            } else {
                let mut file_size: i64 = 0;
                if GetFileSizeEx(handle, &mut file_size).is_ok() && file_size > 0 {
                    file_size as u64
                } else {
                    500 * 1024 * 1024 * 1024 // 500 GB fallback
                }
            }
        };

        info!("Opened Windows volume {} (detected size: {} bytes)", device_path, size);

        Ok(Self {
            handle,
            size,
            sector_size: 512,
        })
    }
}

#[cfg(target_os = "windows")]
impl Drop for WinStorageDevice {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
impl StorageDevice for WinStorageDevice {
    fn size(&self) -> u64 {
        self.size
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        use windows::Win32::Storage::FileSystem::{ReadFile, SetFilePointerEx, FILE_BEGIN};

        if offset >= self.size {
            return Ok(0);
        }

        let mut new_pos: i64 = 0;
        unsafe {
            SetFilePointerEx(self.handle, offset as i64, Some(&mut new_pos), FILE_BEGIN)
                .map_err(|e| StorageError::Io(std::io::Error::from_raw_os_error(e.code().0)))?;
        }

        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(
                self.handle,
                Some(buf),
                Some(&mut bytes_read),
                None,
            )
            .map_err(|e| StorageError::Io(std::io::Error::from_raw_os_error(e.code().0)))?;
        }

        Ok(bytes_read as usize)
    }
}

unsafe impl Send for WinStorageDevice {}
unsafe impl Sync for WinStorageDevice {}

/// List available drives on Windows or system.
pub fn list_drives() -> Result<Vec<DriveDetails>> {
    let mut drives = Vec::new();

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW;

        let mut buf = [0u16; 512];
        let len = unsafe { GetLogicalDriveStringsW(Some(&mut buf)) };
        if len > 0 {
            let strings = &buf[..len as usize];
            for drive_slice in strings.split(|&c| c == 0) {
                if drive_slice.is_empty() {
                    continue;
                }
                let drive_letter = String::from_utf16_lossy(drive_slice);
                let clean_letter = drive_letter.trim_end_matches('\\');
                let dev_path = format!(r"\\.\{clean_letter}");

                drives.push(DriveDetails {
                    device_path: dev_path,
                    display_name: format!("Drive ({clean_letter})"),
                    mount_point: Some(drive_letter),
                    size_bytes: 0,
                    filesystem: Some("NTFS".to_string()),
                    is_read_only: true,
                    is_removable: false,
                });
            }
        }
    }

    Ok(drives)
}
