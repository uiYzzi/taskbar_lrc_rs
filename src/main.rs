use taskbar_lrc::{App, EventLoop};

/// 程序入口点
fn main() -> std::result::Result<(), String> {
    
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
