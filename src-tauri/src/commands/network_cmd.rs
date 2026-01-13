//! 网络相关命令
//!
//! 提供获取本地网络接口信息的功能

use serde::Serialize;
use std::net::{IpAddr, UdpSocket};

/// 网络接口信息
#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    /// 本地回环地址
    pub localhost: String,
    /// 内网 IP 地址（局域网）
    pub lan_ip: Option<String>,
    /// 所有可用的网络接口 IP 地址
    pub all_ips: Vec<String>,
}

/// 获取本地网络信息
///
/// 返回 localhost 和内网 IP 地址，用于客户端连接
#[tauri::command]
pub fn get_network_info() -> Result<NetworkInfo, String> {
    let lan_ip = get_local_ip();
    let all_ips = get_all_local_ips();

    Ok(NetworkInfo {
        localhost: "127.0.0.1".to_string(),
        lan_ip,
        all_ips,
    })
}

/// 获取本机内网 IP 地址
///
/// 通过创建 UDP socket 连接外部地址来获取本机的内网 IP
/// 如果获取到的是 VPN 地址，则从 all_ips 中选择一个合适的
fn get_local_ip() -> Option<String> {
    // 创建一个 UDP socket 并连接到外部地址（不会真正发送数据）
    // 这样可以获取到本机用于出站连接的 IP 地址
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    let ip_str = local_addr.ip().to_string();
    
    // 检查是否是 VPN 地址 (198.18.x.x)
    if let IpAddr::V4(ipv4) = local_addr.ip() {
        if ipv4.octets()[0] == 198 && (ipv4.octets()[1] == 18 || ipv4.octets()[1] == 19) {
            // 是 VPN 地址，尝试从 all_ips 中获取真实的局域网 IP
            let all_ips = get_all_local_ips();
            // 优先选择 192.168.x.x
            if let Some(ip) = all_ips.iter().find(|ip| ip.starts_with("192.168.")) {
                return Some(ip.clone());
            }
            // 其次选择任意私有 IP
            if let Some(ip) = all_ips.first() {
                return Some(ip.clone());
            }
            // 如果没有私有 IP，返回 127.0.0.1
            return Some("127.0.0.1".to_string());
        }
    }
    
    Some(ip_str)
}

/// 获取所有本地网络接口的 IP 地址
///
/// 返回所有非回环的 IPv4 地址，过滤掉 VPN 和虚拟网卡
fn get_all_local_ips() -> Vec<String> {
    let mut ips = Vec::new();

    // 使用 if-addrs crate 获取所有网络接口
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            // 只处理 IPv4 地址
            if let IpAddr::V4(ipv4) = iface.ip() {
                // 过滤掉回环地址
                if ipv4.is_loopback() {
                    continue;
                }

                // 过滤掉链路本地地址 (169.254.x.x)
                if ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 {
                    continue;
                }

                // 过滤掉常见的 VPN 地址段
                // 198.18.0.0/15 (用于基准测试)
                if ipv4.octets()[0] == 198 && (ipv4.octets()[1] == 18 || ipv4.octets()[1] == 19) {
                    continue;
                }

                // 只保留私有网络地址
                // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                let is_private = ipv4.octets()[0] == 10
                    || (ipv4.octets()[0] == 172
                        && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31))
                    || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168);

                if is_private {
                    ips.push(ipv4.to_string());
                }
            }
        }
    }

    ips
}


/// 根据监听地址生成可访问的 URL
///
/// 用于生成客户端配置中的 API URL。
///
/// # 参数
/// - `listen_host`: 服务器监听地址
/// - `port`: 服务器端口
///
/// # 返回
/// - 如果监听地址为 `0.0.0.0`，返回局域网 IP 或 `127.0.0.1`
/// - 如果监听地址为 `127.0.0.1` 或 `localhost`，返回 `127.0.0.1`
/// - 其他情况返回原始地址
pub fn get_accessible_host(listen_host: &str) -> String {
    match listen_host {
        "0.0.0.0" => {
            // 获取局域网 IP，如果没有则使用 127.0.0.1
            get_network_info()
                .ok()
                .and_then(|info| info.lan_ip)
                .unwrap_or_else(|| "127.0.0.1".to_string())
        }
        "localhost" => "127.0.0.1".to_string(),
        _ => listen_host.to_string(),
    }
}

/// 根据监听地址生成可访问的 URL
///
/// # 参数
/// - `listen_host`: 服务器监听地址
/// - `port`: 服务器端口
///
/// # 返回
/// 格式为 `http://{host}:{port}` 的 URL
pub fn get_accessible_url(listen_host: &str, port: u16) -> String {
    let host = get_accessible_host(listen_host);
    format!("http://{}:{}", host, port)
}

/// 根据监听地址生成本地访问的 URL
///
/// 用于 Agent 等本地组件访问服务器。
/// 对于 `0.0.0.0`，返回 `127.0.0.1`（本地访问）。
///
/// # 参数
/// - `listen_host`: 服务器监听地址
/// - `port`: 服务器端口
///
/// # 返回
/// 格式为 `http://{host}:{port}` 的 URL
pub fn get_local_url(listen_host: &str, port: u16) -> String {
    let host = match listen_host {
        "0.0.0.0" | "localhost" => "127.0.0.1".to_string(),
        _ => listen_host.to_string(),
    };
    format!("http://{}:{}", host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_accessible_host_localhost() {
        assert_eq!(get_accessible_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(get_accessible_host("localhost"), "127.0.0.1");
    }

    #[test]
    fn test_get_accessible_host_specific_ip() {
        assert_eq!(get_accessible_host("192.168.1.100"), "192.168.1.100");
        assert_eq!(get_accessible_host("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn test_get_local_url() {
        assert_eq!(get_local_url("0.0.0.0", 8999), "http://127.0.0.1:8999");
        assert_eq!(get_local_url("127.0.0.1", 8999), "http://127.0.0.1:8999");
        assert_eq!(get_local_url("localhost", 8999), "http://127.0.0.1:8999");
        assert_eq!(
            get_local_url("192.168.1.100", 8999),
            "http://192.168.1.100:8999"
        );
    }

    #[test]
    fn test_get_accessible_url_specific_ip() {
        assert_eq!(
            get_accessible_url("192.168.1.100", 8999),
            "http://192.168.1.100:8999"
        );
        assert_eq!(
            get_accessible_url("127.0.0.1", 8999),
            "http://127.0.0.1:8999"
        );
    }
}
