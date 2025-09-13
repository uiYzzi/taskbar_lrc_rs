#![windows_subsystem = "windows"]

use taskbar_lrc::{App, EventLoop};
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// 程序入口点
fn main() -> std::result::Result<(), String> {
    // 检查单实例运行
    let _mutex_handle = ensure_single_instance()
        .map_err(|e| format!("单实例检查失败: {}", e))?;
    
    // 创建应用实例
    let mut app = App::new();
    
    // 创建事件循环
    let event_loop = EventLoop::new()
        .map_err(|e| format!("创建事件循环失败: {}", e))?;
    
    // 运行应用
    event_loop.run_app(&mut app)
        .map_err(|e| format!("运行应用失败: {}", e))?;
    
    Ok(())
}

/// 确保只有单一实例运行
fn ensure_single_instance() -> Result<MutexHandle, String> {
    // 创建一个唯一的互斥锁名称
    let mutex_name = "Global\\TaskbarLrcSingleInstance";
    
    // 将字符串转换为 UTF-16
    let wide_name: Vec<u16> = OsStr::new(mutex_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        // 创建命名互斥锁
        let mutex_handle = CreateMutexW(
            None,
            true, // bInitialOwner = true，立即获取所有权
            PCWSTR(wide_name.as_ptr()),
        ).map_err(|e| format!("创建互斥锁失败: {}", e))?;
        
        // 检查是否已经存在实例
        let last_error = GetLastError();
        if last_error == ERROR_ALREADY_EXISTS {
            // 如果互斥锁已存在，说明已有实例在运行
            let _ = CloseHandle(mutex_handle);
            return Err("应用程序已在运行中，只允许运行一个实例".to_string());
        }
        
        // 返回互斥锁句柄，它会在程序结束时自动释放
        Ok(MutexHandle { handle: mutex_handle })
    }
}

/// 互斥锁句柄包装器，用于自动释放资源
struct MutexHandle {
    handle: HANDLE,
}

impl Drop for MutexHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
