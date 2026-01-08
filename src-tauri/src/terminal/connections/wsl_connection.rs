//! WSL 连接模块（仅 Windows）
//!
//! 提供 Windows Subsystem for Linux (WSL) 连接功能。
//!
//! ## 功能
//! - WSL 发行版列表获取
//! - WSL PTY 创建和管理
//! - 连接状态管理
//! - 终端大小同步
//!
//! ## Requirements
//! - 5.1: 连接到指定的 WSL 发行版
//! - 5.2: 创建 PTY 会话
//! - 5.3: 列出所有可用的 WSL 发行版
//! - 5.4: 连接断开处理和重连
//! - 5.6: 终端大小同步

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use parking_lot::{Mutex, RwLock};

use crate::terminal::{SessionStatus, TerminalOutputEvent, TerminalStatusEvent};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::terminal::block_controller::{BlockInputUnion, BlockMeta};
use crate::terminal::connections::{ConnStatus, ConnectionState};
use crate::terminal::error::TerminalError;
use crate::terminal::events::event_names;
use crate::terminal::persistence::BlockFile;

/// WSL 连接前缀
pub const WSL_CONN_PREFIX: &str = "wsl://";

/// 默认 WSL 发行版
pub const DEFAULT_WSL_DISTRO: &str = "Ubuntu";

// ============================================================================
// WSL 发行版信息
// ============================================================================

/// WSL 发行版信息
///
/// 表示一个已安装的 WSL 发行版。
///
/// _Requirements: 5.3_
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSLDistro {
    /// 发行版名称
    pub name: String,
    /// 是否为默认发行版
    pub is_default: bool,
    /// WSL 版本（1 或 2）
    pub wsl_version: u8,
    /// 发行版状态
    pub state: WSLDistroState,
}

/// WSL 发行版状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WSLDistroState {
    /// 已停止
    Stopped,
    /// 运行中
    Running,
    /// 正在安装
    Installing,
    /// 未知状态
    Unknown,
}

impl Default for WSLDistroState {
    fn default() -> Self {
        Self::Unknown
    }
}

impl std::fmt::Display for WSLDistroState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Running => write!(f, "running"),
            Self::Installing => write!(f, "installing"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// ============================================================================
// WSL 连接选项
// ============================================================================

/// WSL 连接选项
///
/// 存储解析后的 WSL 连接参数。
///
/// ## 格式支持
/// - `wsl://` - 使用默认发行版
/// - `wsl://distro_name` - 指定发行版
/// - `wsl://distro_name/path` - 指定发行版和初始路径
///
/// _Requirements: 5.1_
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WSLOpts {
    /// 发行版名称（None 表示使用默认发行版）
    pub distro: Option<String>,
    /// 初始工作目录
    pub initial_path: Option<String>,
    /// 要执行的命令（None 表示启动交互式 shell）
    pub command: Option<String>,
}

impl Default for WSLOpts {
    fn default() -> Self {
        Self {
            distro: None,
            initial_path: None,
            command: None,
        }
    }
}

impl WSLOpts {
    /// 创建新的 WSL 选项
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置发行版名称
    pub fn with_distro(mut self, distro: impl Into<String>) -> Self {
        self.distro = Some(distro.into());
        self
    }

    /// 设置初始路径
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.initial_path = Some(path.into());
        self
    }

    /// 设置要执行的命令
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// 从连接字符串解析 WSL 选项
    ///
    /// 支持以下格式：
    /// - `wsl://`
    /// - `wsl://distro_name`
    /// - `wsl://distro_name/path`
    ///
    /// # 参数
    /// - `conn_str`: 连接字符串
    ///
    /// # 返回
    /// - `Ok(WSLOpts)`: 解析成功
    /// - `Err(TerminalError)`: 解析失败
    ///
    /// _Requirements: 5.1_
    pub fn parse(conn_str: &str) -> Result<Self, TerminalError> {
        let conn_str = conn_str.trim();

        if conn_str.is_empty() {
            return Err(TerminalError::WSLConnectionFailed(
                "连接字符串不能为空".to_string(),
            ));
        }

        // 检查并移除 wsl:// 前缀
        let path_part = if let Some(stripped) = conn_str.strip_prefix(WSL_CONN_PREFIX) {
            stripped
        } else if conn_str.eq_ignore_ascii_case("wsl") {
            // 支持简单的 "wsl" 格式
            return Ok(Self::default());
        } else {
            return Err(TerminalError::WSLConnectionFailed(format!(
                "无效的 WSL 连接字符串，需要以 '{}' 开头: {}",
                WSL_CONN_PREFIX, conn_str
            )));
        };

        // 如果路径部分为空，使用默认发行版
        if path_part.is_empty() {
            return Ok(Self::default());
        }

        // 解析发行版名称和路径
        let (distro, initial_path) = if let Some(slash_pos) = path_part.find('/') {
            let distro = &path_part[..slash_pos];
            let path = &path_part[slash_pos..];
            (
                if distro.is_empty() {
                    None
                } else {
                    Some(distro.to_string())
                },
                if path.len() > 1 {
                    Some(path.to_string())
                } else {
                    None
                },
            )
        } else {
            (Some(path_part.to_string()), None)
        };

        Ok(Self {
            distro,
            initial_path,
            command: None,
        })
    }

    /// 获取有效的发行版名称
    ///
    /// 如果未指定发行版，返回默认发行版名称。
    pub fn effective_distro(&self) -> &str {
        self.distro.as_deref().unwrap_or(DEFAULT_WSL_DISTRO)
    }

    /// 转换为连接字符串
    ///
    /// _Requirements: 5.1 (Round-Trip)_
    pub fn to_connection_string(&self) -> String {
        let mut result = WSL_CONN_PREFIX.to_string();

        if let Some(ref distro) = self.distro {
            result.push_str(distro);
        }

        if let Some(ref path) = self.initial_path {
            if !path.starts_with('/') {
                result.push('/');
            }
            result.push_str(path);
        }

        result
    }
}

impl std::fmt::Display for WSLOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_connection_string())
    }
}

impl std::str::FromStr for WSLOpts {
    type Err = TerminalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ============================================================================
// WSL 连接管理器
// ============================================================================

/// WSL 连接管理器
///
/// 管理单个 WSL 连接的生命周期，包括连接、断开和重连。
///
/// _Requirements: 5.1, 5.2, 5.4, 5.6_
pub struct WSLConn {
    /// 连接选项
    opts: WSLOpts,
    /// 当前状态
    state: RwLock<ConnectionState>,
    /// 错误信息
    error: RwLock<Option<String>>,
    /// 上次连接时间（Unix 时间戳）
    last_connect_time: AtomicI64,
    /// 活跃连接数
    active_conn_num: AtomicI32,
    /// 是否曾经连接成功
    has_connected: AtomicBool,
    /// wsh 是否启用
    wsh_enabled: AtomicBool,
    /// wsh 版本
    wsh_version: RwLock<Option<String>>,
    /// wsh 错误
    wsh_error: RwLock<Option<String>>,
    /// 不使用 wsh 的原因
    no_wsh_reason: RwLock<Option<String>>,
    /// Tauri 应用句柄（用于事件广播）
    app_handle: RwLock<Option<tauri::AppHandle>>,
}

impl WSLConn {
    /// 创建新的 WSL 连接管理器
    pub fn new(opts: WSLOpts) -> Self {
        Self {
            opts,
            state: RwLock::new(ConnectionState::Init),
            error: RwLock::new(None),
            last_connect_time: AtomicI64::new(0),
            active_conn_num: AtomicI32::new(0),
            has_connected: AtomicBool::new(false),
            wsh_enabled: AtomicBool::new(false),
            wsh_version: RwLock::new(None),
            wsh_error: RwLock::new(None),
            no_wsh_reason: RwLock::new(None),
            app_handle: RwLock::new(None),
        }
    }

    /// 创建带有 Tauri 应用句柄的 WSL 连接管理器
    pub fn with_app_handle(opts: WSLOpts, app_handle: tauri::AppHandle) -> Self {
        let conn = Self::new(opts);
        *conn.app_handle.write() = Some(app_handle);
        conn
    }

    /// 设置 Tauri 应用句柄
    pub fn set_app_handle(&self, app_handle: tauri::AppHandle) {
        *self.app_handle.write() = Some(app_handle);
    }

    /// 从连接字符串创建
    pub fn from_connection_string(conn_str: &str) -> Result<Self, TerminalError> {
        let opts = WSLOpts::parse(conn_str)?;
        Ok(Self::new(opts))
    }

    /// 获取连接选项
    pub fn opts(&self) -> &WSLOpts {
        &self.opts
    }

    /// 获取当前状态
    pub fn state(&self) -> ConnectionState {
        *self.state.read()
    }

    /// 设置状态
    fn set_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write();
        *state = new_state;
    }

    /// 获取错误信息
    pub fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    /// 设置错误信息
    fn set_error(&self, error: Option<String>) {
        let mut err = self.error.write();
        *err = error;
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }

    /// 广播连接状态变更事件
    ///
    /// _Requirements: 7.3_
    fn broadcast_conn_change(&self) {
        use crate::terminal::events::ConnChangeEvent;
        use tauri::Emitter;

        if let Some(ref app_handle) = *self.app_handle.read() {
            let status = self.derive_conn_status();
            let event = ConnChangeEvent {
                connection: self.opts.to_connection_string(),
                status,
            };

            if let Err(e) = app_handle.emit(event_names::CONN_CHANGE, event) {
                tracing::warn!("[WSLConn] 广播连接状态变更事件失败: {}", e);
            }
        }
    }

    /// 派生连接状态
    ///
    /// 生成用于前端显示的连接状态详情。
    ///
    /// _Requirements: 7.1_
    pub fn derive_conn_status(&self) -> ConnStatus {
        ConnStatus {
            status: self.state().to_string(),
            connected: self.is_connected(),
            connection: self.opts.to_connection_string(),
            has_connected: self.has_connected.load(Ordering::SeqCst),
            active_conn_num: self.active_conn_num.load(Ordering::SeqCst),
            error: self.error(),
            wsh_enabled: self.wsh_enabled.load(Ordering::SeqCst),
            wsh_error: self.wsh_error.read().clone(),
            no_wsh_reason: self.no_wsh_reason.read().clone(),
            wsh_version: self.wsh_version.read().clone(),
        }
    }

    /// 连接到 WSL 发行版
    ///
    /// 验证 WSL 是否可用并检查指定的发行版是否存在。
    ///
    /// _Requirements: 5.1, 5.4_
    pub async fn connect(&self) -> Result<(), TerminalError> {
        // 检查状态转换
        let current_state = self.state();
        if !current_state.can_transition_to(ConnectionState::Connecting) {
            return Err(TerminalError::WSLConnectionFailed(format!(
                "无法从 {} 状态开始连接",
                current_state
            )));
        }

        self.set_state(ConnectionState::Connecting);
        self.set_error(None);
        self.broadcast_conn_change();

        let distro = self.opts.effective_distro();
        tracing::info!("[WSLConn] 正在连接到 WSL 发行版: {}", distro);

        // 检查 WSL 是否可用
        if !Self::is_wsl_available() {
            let error_msg = "WSL 不可用，请确保已安装 Windows Subsystem for Linux".to_string();
            tracing::error!("[WSLConn] {}", error_msg);
            self.set_state(ConnectionState::Error);
            self.set_error(Some(error_msg.clone()));
            self.broadcast_conn_change();
            return Err(TerminalError::WSLConnectionFailed(error_msg));
        }

        // 检查发行版是否存在
        let distros = Self::list_distros()?;
        let distro_exists = distros.iter().any(|d| d.name.eq_ignore_ascii_case(distro));

        if !distro_exists {
            let available: Vec<_> = distros.iter().map(|d| d.name.as_str()).collect();
            let error_msg = format!(
                "WSL 发行版 '{}' 不存在。可用的发行版: {:?}",
                distro, available
            );
            tracing::error!("[WSLConn] {}", error_msg);
            self.set_state(ConnectionState::Error);
            self.set_error(Some(error_msg.clone()));
            self.broadcast_conn_change();
            return Err(TerminalError::WSLConnectionFailed(error_msg));
        }

        // 连接成功
        self.set_state(ConnectionState::Connected);
        self.has_connected.store(true, Ordering::SeqCst);
        self.last_connect_time
            .store(chrono::Utc::now().timestamp(), Ordering::SeqCst);
        self.active_conn_num.fetch_add(1, Ordering::SeqCst);
        self.broadcast_conn_change();

        tracing::info!("[WSLConn] 已连接到 WSL 发行版: {}", distro);
        Ok(())
    }

    /// 断开连接
    ///
    /// _Requirements: 5.4_
    pub async fn disconnect(&self) -> Result<(), TerminalError> {
        tracing::info!("[WSLConn] 断开 WSL 连接: {}", self.opts.effective_distro());

        self.set_state(ConnectionState::Disconnected);
        self.active_conn_num.fetch_sub(1, Ordering::SeqCst);
        self.broadcast_conn_change();

        Ok(())
    }

    /// 检查 WSL 是否可用
    ///
    /// 通过执行 `wsl --status` 命令检查 WSL 是否已安装并可用。
    #[cfg(target_os = "windows")]
    pub fn is_wsl_available() -> bool {
        use std::process::Command;

        match Command::new("wsl").arg("--status").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// 检查 WSL 是否可用（非 Windows 平台始终返回 false）
    #[cfg(not(target_os = "windows"))]
    pub fn is_wsl_available() -> bool {
        false
    }

    /// 列出所有可用的 WSL 发行版
    ///
    /// _Requirements: 5.3_
    #[cfg(target_os = "windows")]
    pub fn list_distros() -> Result<Vec<WSLDistro>, TerminalError> {
        use std::process::Command;

        let output = Command::new("wsl")
            .args(["--list", "--verbose"])
            .output()
            .map_err(|e| {
                TerminalError::WSLConnectionFailed(format!("执行 wsl --list 失败: {}", e))
            })?;

        if !output.status.success() {
            return Err(TerminalError::WSLConnectionFailed(
                "wsl --list 命令执行失败".to_string(),
            ));
        }

        // 解析输出
        // 输出格式类似：
        //   NAME                   STATE           VERSION
        // * Ubuntu                 Running         2
        //   Debian                 Stopped         2
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut distros = Vec::new();

        for line in stdout.lines().skip(1) {
            // 跳过标题行
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let is_default = line.starts_with('*');
            let line = line.trim_start_matches('*').trim();

            // 解析各列
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let state = match parts[1].to_lowercase().as_str() {
                    "running" => WSLDistroState::Running,
                    "stopped" => WSLDistroState::Stopped,
                    "installing" => WSLDistroState::Installing,
                    _ => WSLDistroState::Unknown,
                };
                let wsl_version = parts[2].parse().unwrap_or(2);

                distros.push(WSLDistro {
                    name,
                    is_default,
                    wsl_version,
                    state,
                });
            }
        }

        Ok(distros)
    }

    /// 列出所有可用的 WSL 发行版（非 Windows 平台返回空列表）
    #[cfg(not(target_os = "windows"))]
    pub fn list_distros() -> Result<Vec<WSLDistro>, TerminalError> {
        Ok(Vec::new())
    }

    /// 获取默认 WSL 发行版
    ///
    /// _Requirements: 5.3_
    pub fn get_default_distro() -> Result<Option<WSLDistro>, TerminalError> {
        let distros = Self::list_distros()?;
        Ok(distros.into_iter().find(|d| d.is_default))
    }
}

// ============================================================================
// WSL Shell 进程
// ============================================================================

/// WSL Shell 进程封装
///
/// 封装 WSL PTY 进程，提供输入输出和生命周期管理。
///
/// _Requirements: 5.2, 5.6_
pub struct WSLShellProc {
    /// Block ID
    block_id: String,
    /// WSL 选项
    #[allow(dead_code)]
    opts: WSLOpts,
    /// PTY 写入器
    #[cfg(target_os = "windows")]
    writer: Arc<parking_lot::Mutex<Box<dyn std::io::Write + Send>>>,
    /// PTY Master（用于调整大小）
    #[cfg(target_os = "windows")]
    master: Arc<parking_lot::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    /// 关闭标志
    shutdown_flag: Arc<AtomicBool>,
    /// 进程退出码
    exit_code: Arc<AtomicI32>,
    /// 是否已退出
    exited: Arc<AtomicBool>,
}

impl WSLShellProc {
    /// 创建新的 WSL Shell 进程
    ///
    /// # 参数
    /// - `block_id`: Block ID
    /// - `opts`: WSL 连接选项
    /// - `rows`: 终端行数
    /// - `cols`: 终端列数
    /// - `app_handle`: Tauri 应用句柄
    /// - `block_meta`: 块元数据配置
    /// - `input_rx`: 输入接收器
    /// - `block_file`: 块文件存储（可选）
    ///
    /// # 返回
    /// - `Ok(WSLShellProc)`: 创建成功
    /// - `Err(TerminalError)`: 创建失败
    ///
    /// _Requirements: 5.2_
    #[cfg(target_os = "windows")]
    pub async fn new(
        block_id: String,
        opts: WSLOpts,
        rows: u16,
        cols: u16,
        app_handle: tauri::AppHandle,
        block_meta: BlockMeta,
        input_rx: mpsc::Receiver<BlockInputUnion>,
        block_file: Option<Arc<BlockFile>>,
    ) -> Result<Self, TerminalError> {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};

        tracing::info!(
            "[WSLShellProc] 创建 WSL 进程: block_id={}, distro={}, size={}x{}",
            block_id,
            opts.effective_distro(),
            cols,
            rows
        );

        let pty_system = native_pty_system();

        // 创建 PTY
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::PtyCreationFailed(e.to_string()))?;

        // 构建 WSL 命令
        let cmd = Self::build_wsl_command(&opts, &block_meta)?;

        // 启动子进程
        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::PtyCreationFailed(e.to_string()))?;

        // 获取写入器
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TerminalError::PtyCreationFailed(e.to_string()))?;

        // 获取读取器
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::PtyCreationFailed(e.to_string()))?;

        // 创建共享状态
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let exit_code = Arc::new(AtomicI32::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let writer = Arc::new(Mutex::new(writer));
        let master = Arc::new(Mutex::new(pair.master));

        // 启动输出读取任务
        Self::spawn_output_reader(
            block_id.clone(),
            reader,
            app_handle.clone(),
            shutdown_flag.clone(),
            exit_code.clone(),
            exited.clone(),
            block_file,
        );

        // 启动输入处理任务
        Self::spawn_input_handler(
            block_id.clone(),
            writer.clone(),
            master.clone(),
            input_rx,
            shutdown_flag.clone(),
        );

        tracing::info!("[WSLShellProc] WSL 进程已创建: block_id={}", block_id);

        Ok(Self {
            block_id,
            opts,
            writer,
            master,
            shutdown_flag,
            exit_code,
            exited,
        })
    }

    /// 创建新的 WSL Shell 进程（非 Windows 平台）
    #[cfg(not(target_os = "windows"))]
    pub async fn new(
        block_id: String,
        opts: WSLOpts,
        _rows: u16,
        _cols: u16,
        _app_handle: tauri::AppHandle,
        _block_meta: BlockMeta,
        _input_rx: mpsc::Receiver<BlockInputUnion>,
        _block_file: Option<Arc<BlockFile>>,
    ) -> Result<Self, TerminalError> {
        // 在非 Windows 平台上，创建一个占位结构但返回错误
        // 这样可以保持 API 一致性
        let _ = Self {
            block_id,
            opts,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            exit_code: Arc::new(AtomicI32::new(0)),
            exited: Arc::new(AtomicBool::new(false)),
        };
        Err(TerminalError::WSLConnectionFailed(
            "WSL 仅在 Windows 平台上可用".to_string(),
        ))
    }

    /// 构建 WSL 命令
    ///
    /// _Requirements: 5.1, 5.2_
    #[cfg(target_os = "windows")]
    fn build_wsl_command(
        opts: &WSLOpts,
        block_meta: &BlockMeta,
    ) -> Result<portable_pty::CommandBuilder, TerminalError> {
        use portable_pty::CommandBuilder;

        let mut cmd = CommandBuilder::new("wsl.exe");

        // 指定发行版
        if let Some(ref distro) = opts.distro {
            cmd.arg("-d");
            cmd.arg(distro);
        }

        // 指定工作目录
        if let Some(ref path) = opts.initial_path {
            cmd.arg("--cd");
            cmd.arg(path);
        } else if let Some(ref cwd) = block_meta.cmd_cwd {
            cmd.arg("--cd");
            cmd.arg(cwd);
        }

        // 如果有命令要执行
        if let Some(ref command) = opts.command {
            cmd.arg("--");
            cmd.arg(command);
        } else if let Some(ref meta_cmd) = block_meta.cmd {
            cmd.arg("--");
            cmd.arg(meta_cmd);
            if let Some(ref args) = block_meta.cmd_args {
                for arg in args {
                    cmd.arg(arg);
                }
            }
        }

        // 设置环境变量
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        if let Some(ref env_vars) = block_meta.cmd_env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        Ok(cmd)
    }

    /// 启动输出读取任务
    #[cfg(target_os = "windows")]
    fn spawn_output_reader(
        block_id: String,
        mut reader: Box<dyn Read + Send>,
        app_handle: tauri::AppHandle,
        shutdown_flag: Arc<AtomicBool>,
        exit_code: Arc<AtomicI32>,
        exited: Arc<AtomicBool>,
        block_file: Option<Arc<BlockFile>>,
    ) {
        use tauri::Emitter;

        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];

            loop {
                if shutdown_flag.load(Ordering::Relaxed) {
                    tracing::debug!("[WSLShellProc] 收到关闭信号: block_id={}", block_id);
                    break;
                }

                match reader.read(&mut buffer) {
                    Ok(0) => {
                        tracing::info!("[WSLShellProc] WSL 进程已退出: block_id={}", block_id);
                        exited.store(true, Ordering::SeqCst);

                        let _ = app_handle.emit(
                            event_names::TERMINAL_STATUS,
                            TerminalStatusEvent {
                                session_id: block_id.clone(),
                                status: SessionStatus::Done,
                                exit_code: Some(exit_code.load(Ordering::SeqCst)),
                                error: None,
                            },
                        );
                        break;
                    }
                    Ok(n) => {
                        let output_data = &buffer[..n];

                        if let Some(ref bf) = block_file {
                            if let Err(e) = bf.append_data(output_data) {
                                tracing::warn!(
                                    "[WSLShellProc] 写入块文件失败: block_id={}, error={}",
                                    block_id,
                                    e
                                );
                            }
                        }

                        let data = BASE64.encode(output_data);
                        let _ = app_handle.emit(
                            event_names::TERMINAL_OUTPUT,
                            TerminalOutputEvent {
                                session_id: block_id.clone(),
                                data,
                            },
                        );
                    }
                    Err(e) => {
                        if shutdown_flag.load(Ordering::Relaxed) {
                            break;
                        }

                        tracing::error!(
                            "[WSLShellProc] 读取错误: block_id={}, error={}",
                            block_id,
                            e
                        );
                        exited.store(true, Ordering::SeqCst);

                        let _ = app_handle.emit(
                            event_names::TERMINAL_STATUS,
                            TerminalStatusEvent {
                                session_id: block_id.clone(),
                                status: SessionStatus::Error,
                                exit_code: None,
                                error: Some(e.to_string()),
                            },
                        );
                        break;
                    }
                }
            }
        });
    }

    /// 启动输入处理任务
    #[cfg(target_os = "windows")]
    fn spawn_input_handler(
        block_id: String,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
        mut input_rx: mpsc::Receiver<BlockInputUnion>,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        use portable_pty::PtySize;

        tokio::spawn(async move {
            while let Some(input) = input_rx.recv().await {
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }

                // 处理输入数据
                if let Some(data) = &input.input_data {
                    let mut w = writer.lock();
                    if let Err(e) = w.write_all(data) {
                        tracing::error!(
                            "[WSLShellProc] 写入失败: block_id={}, error={}",
                            block_id,
                            e
                        );
                        continue;
                    }
                    if let Err(e) = w.flush() {
                        tracing::error!(
                            "[WSLShellProc] Flush 失败: block_id={}, error={}",
                            block_id,
                            e
                        );
                    }
                }

                // 处理终端大小调整
                if let Some(size) = &input.term_size {
                    let m = master.lock();
                    if let Err(e) = m.resize(PtySize {
                        rows: size.rows,
                        cols: size.cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    }) {
                        tracing::error!(
                            "[WSLShellProc] 调整大小失败: block_id={}, error={}",
                            block_id,
                            e
                        );
                    } else {
                        tracing::debug!(
                            "[WSLShellProc] 调整大小: block_id={}, size={}x{}",
                            block_id,
                            size.cols,
                            size.rows
                        );
                    }
                }

                // 处理信号
                if let Some(sig_name) = &input.sig_name {
                    tracing::debug!(
                        "[WSLShellProc] 收到信号: block_id={}, signal={}",
                        block_id,
                        sig_name
                    );
                }
            }

            tracing::debug!("[WSLShellProc] 输入处理任务结束: block_id={}", block_id);
        });
    }

    /// 获取 Block ID
    pub fn block_id(&self) -> &str {
        &self.block_id
    }

    /// 获取 WSL 选项
    pub fn opts(&self) -> &WSLOpts {
        &self.opts
    }

    /// 检查进程是否已退出
    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// 获取退出码
    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::SeqCst)
    }

    /// 写入数据到 PTY
    #[cfg(target_os = "windows")]
    pub fn write(&self, data: &[u8]) -> Result<(), TerminalError> {
        let mut writer = self.writer.lock();
        writer
            .write_all(data)
            .map_err(|e| TerminalError::WriteFailed(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| TerminalError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    /// 写入数据到 PTY（非 Windows 平台）
    #[cfg(not(target_os = "windows"))]
    pub fn write(&self, _data: &[u8]) -> Result<(), TerminalError> {
        Err(TerminalError::WSLConnectionFailed(
            "WSL 仅在 Windows 平台上可用".to_string(),
        ))
    }

    /// 调整 PTY 大小
    ///
    /// _Requirements: 5.6_
    #[cfg(target_os = "windows")]
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), TerminalError> {
        use portable_pty::PtySize;

        let master = self.master.lock();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::ResizeFailed(e.to_string()))?;
        tracing::debug!(
            "[WSLShellProc] 调整大小: block_id={}, size={}x{}",
            self.block_id,
            cols,
            rows
        );
        Ok(())
    }

    /// 调整 PTY 大小（非 Windows 平台）
    #[cfg(not(target_os = "windows"))]
    pub fn resize(&self, _rows: u16, _cols: u16) -> Result<(), TerminalError> {
        Err(TerminalError::WSLConnectionFailed(
            "WSL 仅在 Windows 平台上可用".to_string(),
        ))
    }

    /// 优雅终止进程
    pub async fn terminate(&self) -> i32 {
        tracing::info!(
            "[WSLShellProc] 优雅终止 WSL 进程: block_id={}",
            self.block_id
        );
        self.shutdown_flag.store(true, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        self.exit_code.load(Ordering::SeqCst)
    }

    /// 强制终止进程
    pub async fn kill(&self) {
        tracing::info!(
            "[WSLShellProc] 强制终止 WSL 进程: block_id={}",
            self.block_id
        );
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }
}

impl Drop for WSLShellProc {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        tracing::debug!("[WSLShellProc] WSL 进程已销毁: block_id={}", self.block_id);
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 检查连接名称是否为 WSL 连接
///
/// WSL 连接名称以 "wsl://" 开头或等于 "wsl"。
pub fn is_wsl_conn_name(conn_name: &str) -> bool {
    let conn_name = conn_name.trim().to_lowercase();
    conn_name.starts_with(WSL_CONN_PREFIX) || conn_name == "wsl"
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // WSLOpts 解析测试
    mod wsl_opts_tests {
        use super::*;

        #[test]
        fn test_parse_empty_wsl() {
            let opts = WSLOpts::parse("wsl://").unwrap();
            assert!(opts.distro.is_none());
            assert!(opts.initial_path.is_none());
            assert!(opts.command.is_none());
        }

        #[test]
        fn test_parse_simple_wsl() {
            let opts = WSLOpts::parse("wsl").unwrap();
            assert!(opts.distro.is_none());
            assert!(opts.initial_path.is_none());
        }

        #[test]
        fn test_parse_with_distro() {
            let opts = WSLOpts::parse("wsl://Ubuntu").unwrap();
            assert_eq!(opts.distro, Some("Ubuntu".to_string()));
            assert!(opts.initial_path.is_none());
        }

        #[test]
        fn test_parse_with_distro_and_path() {
            let opts = WSLOpts::parse("wsl://Ubuntu/home/user").unwrap();
            assert_eq!(opts.distro, Some("Ubuntu".to_string()));
            assert_eq!(opts.initial_path, Some("/home/user".to_string()));
        }

        #[test]
        fn test_parse_debian() {
            let opts = WSLOpts::parse("wsl://Debian").unwrap();
            assert_eq!(opts.distro, Some("Debian".to_string()));
        }

        #[test]
        fn test_parse_invalid_prefix() {
            let result = WSLOpts::parse("ssh://user@host");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_empty_string() {
            let result = WSLOpts::parse("");
            assert!(result.is_err());
        }

        #[test]
        fn test_effective_distro_default() {
            let opts = WSLOpts::default();
            assert_eq!(opts.effective_distro(), DEFAULT_WSL_DISTRO);
        }

        #[test]
        fn test_effective_distro_specified() {
            let opts = WSLOpts::new().with_distro("Debian");
            assert_eq!(opts.effective_distro(), "Debian");
        }

        #[test]
        fn test_to_connection_string_empty() {
            let opts = WSLOpts::default();
            assert_eq!(opts.to_connection_string(), "wsl://");
        }

        #[test]
        fn test_to_connection_string_with_distro() {
            let opts = WSLOpts::new().with_distro("Ubuntu");
            assert_eq!(opts.to_connection_string(), "wsl://Ubuntu");
        }

        #[test]
        fn test_to_connection_string_with_path() {
            let opts = WSLOpts::new().with_distro("Ubuntu").with_path("/home/user");
            assert_eq!(opts.to_connection_string(), "wsl://Ubuntu/home/user");
        }

        #[test]
        fn test_round_trip_simple() {
            let original = "wsl://Ubuntu";
            let opts = WSLOpts::parse(original).unwrap();
            assert_eq!(opts.to_connection_string(), original);
        }

        #[test]
        fn test_round_trip_with_path() {
            let original = "wsl://Debian/home/user";
            let opts = WSLOpts::parse(original).unwrap();
            assert_eq!(opts.to_connection_string(), original);
        }

        #[test]
        fn test_builder_pattern() {
            let opts = WSLOpts::new()
                .with_distro("Ubuntu")
                .with_path("/home/user")
                .with_command("ls -la");

            assert_eq!(opts.distro, Some("Ubuntu".to_string()));
            assert_eq!(opts.initial_path, Some("/home/user".to_string()));
            assert_eq!(opts.command, Some("ls -la".to_string()));
        }

        #[test]
        fn test_from_str() {
            let opts: WSLOpts = "wsl://Ubuntu".parse().unwrap();
            assert_eq!(opts.distro, Some("Ubuntu".to_string()));
        }

        #[test]
        fn test_display() {
            let opts = WSLOpts::new().with_distro("Ubuntu");
            assert_eq!(format!("{}", opts), "wsl://Ubuntu");
        }
    }

    // is_wsl_conn_name 测试
    mod is_wsl_conn_name_tests {
        use super::*;

        #[test]
        fn test_wsl_prefix() {
            assert!(is_wsl_conn_name("wsl://Ubuntu"));
            assert!(is_wsl_conn_name("wsl://Debian"));
            assert!(is_wsl_conn_name("wsl://"));
        }

        #[test]
        fn test_simple_wsl() {
            assert!(is_wsl_conn_name("wsl"));
            assert!(is_wsl_conn_name("WSL"));
            assert!(is_wsl_conn_name("Wsl"));
        }

        #[test]
        fn test_not_wsl() {
            assert!(!is_wsl_conn_name("ssh://user@host"));
            assert!(!is_wsl_conn_name("local"));
            assert!(!is_wsl_conn_name(""));
            assert!(!is_wsl_conn_name("wslx"));
        }

        #[test]
        fn test_with_whitespace() {
            assert!(is_wsl_conn_name("  wsl://Ubuntu  "));
            assert!(is_wsl_conn_name("  wsl  "));
        }
    }

    // WSLDistroState 测试
    mod distro_state_tests {
        use super::*;

        #[test]
        fn test_default() {
            let state = WSLDistroState::default();
            assert_eq!(state, WSLDistroState::Unknown);
        }

        #[test]
        fn test_display() {
            assert_eq!(format!("{}", WSLDistroState::Running), "running");
            assert_eq!(format!("{}", WSLDistroState::Stopped), "stopped");
            assert_eq!(format!("{}", WSLDistroState::Installing), "installing");
            assert_eq!(format!("{}", WSLDistroState::Unknown), "unknown");
        }
    }

    // WSLConn 测试
    mod wsl_conn_tests {
        use super::*;

        #[test]
        fn test_new() {
            let opts = WSLOpts::new().with_distro("Ubuntu");
            let conn = WSLConn::new(opts);

            assert_eq!(conn.state(), ConnectionState::Init);
            assert!(!conn.is_connected());
            assert!(conn.error().is_none());
        }

        #[test]
        fn test_from_connection_string() {
            let conn = WSLConn::from_connection_string("wsl://Ubuntu").unwrap();
            assert_eq!(conn.opts().distro, Some("Ubuntu".to_string()));
        }

        #[test]
        fn test_derive_conn_status() {
            let opts = WSLOpts::new().with_distro("Ubuntu");
            let conn = WSLConn::new(opts);

            let status = conn.derive_conn_status();
            assert_eq!(status.status, "init");
            assert!(!status.connected);
            assert_eq!(status.connection, "wsl://Ubuntu");
            assert!(!status.has_connected);
        }

        #[test]
        fn test_is_wsl_available_non_windows() {
            // 在非 Windows 平台上应该返回 false
            #[cfg(not(target_os = "windows"))]
            {
                assert!(!WSLConn::is_wsl_available());
            }
        }

        #[test]
        fn test_list_distros_non_windows() {
            // 在非 Windows 平台上应该返回空列表
            #[cfg(not(target_os = "windows"))]
            {
                let distros = WSLConn::list_distros().unwrap();
                assert!(distros.is_empty());
            }
        }
    }
}
