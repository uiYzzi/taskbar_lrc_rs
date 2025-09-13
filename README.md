# Taskbar LRC

一个简洁的 Windows 任务栏歌词显示器，在不干扰其他操作的情况下为您提供实时歌词展示。

## ✨ 特性

- 🎵 **实时歌词显示** - 在任务栏上同步显示当前播放歌曲的歌词
- 🎨 **逐字滚动效果** - 流畅的歌词滚动和高亮显示
- 🎶 **多平台支持** - 支持网易云音乐、QQ音乐等主流音乐平台
- ⚡ **高性能优化** - 按需重绘和智能资源管理，低CPU占用
- 🖥️ **任务栏集成** - 无缝集成到Windows任务栏，不影响正常使用

## 🚀 快速开始

### 环境要求

- Windows 10 或更高版本
- Rust 1.68+ (推荐使用最新稳定版)

### 编译和运行

```bash
# 克隆项目
git clone https://github.com/your-username/taskbar_lrc.git
cd taskbar_lrc

# 编译项目
cargo build --release

# 运行程序
cargo run
```

## 🛠️ 技术栈

- **语言**: Rust 2024 Edition
- **UI框架**: winit + softbuffer
- **字体渲染**: fontdue
- **异步运行时**: tokio
- **网络请求**: reqwest
- **系统集成**: Windows API (windows crate)

## 📁 项目结构

```
src/
├── font/           # 字体加载与渲染
├── graphics/       # 图形渲染模块
├── lyrics/         # 歌词获取与管理
├── system/         # 系统交互与媒体监控
├── window/         # 窗口管理与定位
├── app.rs         # 应用程序主逻辑
└── widget.rs      # 歌词显示组件
```

## 🎯 核心功能

- **智能歌词同步**: 自动检测系统播放状态，实时同步歌词显示
- **按需渲染**: 只有在内容变化时才触发重绘，优化性能
- **自适应更新**: 根据播放状态动态调整更新频率
- **多音乐平台**: 支持多个音乐平台的歌词获取API

## 📄 许可证

本项目采用 GPL-3.0 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

---

**注意**: 本项目仅在Windows系统上运行，依赖Windows特有的API和任务栏集成功能。