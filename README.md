# NexDNS: 高性能自研原生统一单内核 DNS 服务系统

[![Release & Publish](https://github.com/aurorax-neo/NexDNS/actions/workflows/release.yml/badge.svg)](https://github.com/aurorax-neo/NexDNS/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/aurorax-neo/NexDNS?color=cyan)](https://github.com/aurorax-neo/NexDNS/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

NexDNS 是一款自研原生统一单内核架构的下一代高性能 DNS 综合解决方案。深度融合主流开源 DNS 工具的核心能力：
- **AdGuard Home 风格**：全兼容 ABP 规则语法（`||domain^`、`@@||`）、Hosts 格式与正则的高并发安全拦截；
- **MosDNS 风格**：高度灵活的流水线分流调度、GeoSite / CIDR 地址树匹配与双重防 DNS 污染探测；
- **SmartDNS 风格**：高性能内存缓存、热点静默预取与基于真实 TCP 80/443 建连时延的最优 IP 测速优选；
- **现代化独立工作台**：采用 React 19 + Tailwind 构建的高级中性底色管理面板，单二进制内嵌交付。

基于 **Rust 异步高性能单内核** 构建，彻底消除传统多容器、多进程套娃（53 -> 4053 -> 5053 -> 6053）带来的跨进程 IPC 开销、内存暴涨与网络跳数延迟。

---

## 核心特性

1. **统一自研原生单内核**：
   - 零跨进程转发与网络跳数损耗，内存零拷贝流水线执行。
   - 客户端来源 IP 与全流程耗时纳秒级贯穿，日志记录 100% 真实。
   - 单二进制交付，前端 SPA 静态资源由 Rust 内部原生内嵌服务。

2. **智能分流与防污染一体化引擎 (Unified Smart Routing & Zero-Proxy Anti-Pollution)**：
   - **单一总开关统一联动**：域名智能分流调度与非大陆 IP 防污染探测深度融合（`routing.enabled` 默认开启）。
   - **最长后缀与精确特异度仲裁**：自动比对 `direct.txt` 与 `proxy.txt` 匹配粒度（如 `proxy.txt` 的 `domain:google.cn` 自动优先于 `direct.txt` 的 `domain:cn`；`direct.txt` 的 `full:dl.l.google.com` 自动优先于 `domain:google.com`），彻底消除 `.cn` 海外托管/跳转域名误走大陆直连。
   - **无代理直连防污染备用链 (`gfw -> overseas`)**：内置大陆公网可直接建立加密 TLS HTTP/2 连接的未封锁海外纯净 DoH 节点组（AdGuard DNS、台湾 HiNet、TWNIC Quad101、Quad9）。无论在无代理环境还是跨境代理通道瞬时抖动窗口，均能无缝获取无污染真实境外 IP。
   - **零大陆回退与防缓存投毒硬隔离**：代理域与检出非中国 IP 污染的冷门域**严禁回退 `mainland` 大陆上游、严禁将大陆污染应答写入缓存**；境外组应答缓存 TTL 自动封顶 `<= 300s`，确保线路异常恢复后快速收敛。

3. **高效威胁与安全规则拦截 (Security Filtering)**：
   - 兼容 Adblock Plus (ABP) 规则语法（`||domain^`、`@@||whitelist^`）、Hosts 格式及正则表达式。
   - 支持 ZeroIP (`0.0.0.0` / `::`)、NXDOMAIN 与 REFUSED 阻断模式。
   - 内置域名规则命中实时诊断工具。

4. **并发查询与最优测速 (Fastest IP & SmartCache)**：
   - 支持 UDP 53、TCP 53、DoT (`tls://`，全局共享 TLS 会话配置与长连接池复用) 以及基于 **HTTP/2 多路复用长连接** 的加密 DNS over HTTPS (DoH)。
   - **Singleflight 并发去重 & 阶梯式 Hedged Request**：同一 `(qname, qtype)` 在途冷查询自动合并去重；上游组优先单发 EWMA 历史最低延迟节点，仅在超时（如 150ms）未回时阶梯补发备节点，彻底消除代理连接风暴与流量倍增。
   - **TCP 端口建连测速优选**：针对国内直连返回的多条 IP 记录异步并发握手（支持 `tcp_80_443` / `tcp_443` / `tcp_80`，默认跳过境外代理组并内置有界缓存与并发限流），将最低 RTT 的最快 IP 置顶返回客户端。
   - **标准智能缓存与安全预取**：默认缓存范围 `[60s, 1800s]`，80% TTL 静默预取完整复用智能分流 + 防污染 + 地址族过滤流水线，绝不将污染 IP 写回缓存。
   - **持久化高优先级自定义分流覆盖**：支持在 `config.yaml` 与控制台配置 `routing.custom_proxy_domains` / `routing.custom_direct_domains`，优先级高于自动更新的 `direct.txt` / `proxy.txt`（完美解决托管在海外 CDN 的 `.cn` 域名分流）。

5. **服务端口物理隔离与原生诊断工具**：
   - **端口物理隔离**：UDP/TCP 53（标准 DNS 解析）、TCP 8053（RFC 8484 独立 DoH 解析）、TCP 3000（Web 控制台与 API）。
   - **原生 DNS 查询诊断工具 (Query Tool)**：直接在控制台页面输入任意域名，模拟真实内核流水线解析，直观展示决策链路、应答记录与耗时。
   - **分钟级日志滚动保留**：支持按分钟（min）控制日志留存（默认 60 分钟），定时清退并强制执行 `PRAGMA wal_checkpoint(TRUNCATE)` 与 `VACUUM`，磁盘空间即时归还系统。

6. **现代专业工作台 (React 19)**：
   - 严格遵循现代工作台设计规范，适配深浅色模式（中性深色底色，发丝边框）。
   - 表面层级细分（1%~3% 明度差，深色模式字重微调防膨胀）。
   - 数字与时延防抖（`font-variant-numeric: tabular-nums`）。
   - 规范化浮层：Modal（400/520/720px 三档）、Drawer（深度诊断报文）、Popconfirm（行内操作）、AlertDialog（高危清空）、Snackbar（可撤销删除）。
   - URL 驱动的无抖动 Tab 状态持久化。

---

## 快速开始

### 默认控制台登录凭据

- **访问地址**：`http://<服务器IP>:3000`
- **默认用户名**：`admin`
- **默认密码**：`admin1234`
- **修改与重置说明**：首次登录后请及时前往控制台「系统设置」修改管理员用户名与密码（凭据使用 Argon2id 哈希持久化存储于 `data/config.yaml` 的 `auth` 字段）。若忘记密码，可直接将 `data/config.yaml` 中的 `auth.password` 改为新明文密码并重启服务，启动后会自动平滑升级为 Argon2id 哈希。

### 1. 编译并运行单二进制服务

```bash
# 1. 构建前端 SPA
cd frontend
pnpm install
pnpm build

# 2. 编译 Rust 后端（自动内嵌前端 dist 产物）
cd ../backend
cargo build --release

# 3. 运行服务（默认监听 UDP/TCP 53 以及 Web 控制台 3000）
./target/release/nexdns
```

### 方式一：直接下载各平台预编译静态单二进制 (开箱即用)

前往 [GitHub Releases](https://github.com/aurorax-neo/NexDNS/releases) 页面直接下载对应平台架构的独立单二进制压缩包（已完整内嵌管理控制台前端 SPA，无需安装任何运行时依赖）：

| 操作系统 / 架构 | Target 标识 | 适用平台说明 |
| :--- | :--- | :--- |
| **Linux x86_64** | `x86_64-unknown-linux-musl` | 常见 Linux 64位物理机、云服务器、软路由 (纯静态零依赖) |
| **Linux ARM64** | `aarch64-unknown-linux-musl` | 树莓派 4/5、甲骨文 ARM、各类 ARM64 软路由与工控机 |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | 苹果 M1 / M2 / M3 / M4 系列芯片原生架构 |
| **macOS (Intel)** | `x86_64-apple-darwin` | 苹果 Intel 系列处理器 Mac 设备 |
| **Windows x64** | `x86_64-pc-windows-msvc` | Windows 10 / 11 / Server 64 位系统 |

运行命令：
```bash
# 解压后直接启动
chmod +x nexdns
./nexdns --config data/config.yaml
```

### 方式二：Docker Compose 多架构镜像部署 (推荐生产模式)

官方通过 GitHub Actions 自动化编译并发布多架构镜像至 GitHub Container Registry（同时原生支持 `linux/amd64` 与 `linux/arm64`）：

创建 `docker-compose.yml`：

```yaml
services:
  nexdns:
    image: ghcr.io/aurorax-neo/nexdns:latest
    container_name: nexdns
    restart: unless-stopped
    ports:
      - "53:53/udp"
      - "53:53/tcp"
      - "3000:3000/tcp"
      - "8053:8053/tcp"
    volumes:
      - ./data:/app/data
    environment:
      - TZ=Asia/Shanghai
```

启动服务：
```bash
docker compose up -d
```

打开浏览器访问 `http://<服务器IP>:3000` 即可使用（默认账号 `admin`，默认密码 `admin1234`）。

### 方式三：源码本地自主编译构建

环境要求：Rust 1.80+、Node.js 20+、pnpm。

```bash
# 1. 克隆源码仓库
git clone https://github.com/aurorax-neo/NexDNS.git
cd NexDNS

# 2. 构建前端静态资源
cd frontend
pnpm install
pnpm build

# 3. 编译 Rust 后端 (自动内嵌前端 dist 静态产物)
cd ../backend
cargo build --release

# 4. 运行单二进制文件
./target/release/nexdns --config data/config.yaml
```

---

## 生产环境最佳实践（无 IPv6 出口必开项）

- **【无 IPv6 出口必开项】开启仅 IPv4 下游响应短路 (`dns.response_mode: ipv4_only`)**：
  在纯 IPv4 宽带、Docker 默认桥接或代理无 IPv6 出口的环境中，前往控制台 **「系统设置 -> DNS 基础监听与下游服务」** 点击 **「一键开启仅 IPv4 优化」**（或在 `data/config.yaml` 设置 `dns.response_mode: ipv4_only` 与 `dns.connect_ip_version: ipv4`）。内核将在缓存前置阶段以 **0.05ms** 直接对 `AAAA` 查询返回标准 `NoError (NODATA)` 并附带 `Authority SOA` 否定缓存记录，彻底消除双栈 Happy Eyeballs 退避卡顿并**立即节省 50% 上游查询量**。
- **托管在 Cloudflare 等海外 CDN 的 `.cn` 域名覆盖 (`routing.custom_proxy_domains`)**：
  由于社区 `direct.txt` 默认包含 `.cn` 顶级域，若个别 `.cn` 域名托管于海外 CDN 需要防污染解析，可在控制台 **「分流规则 -> 自定义强制分流覆盖规则」** 中填入该域名，优先级高于订阅规则且不会被定时自动更新覆盖。

---

## 核心 RESTful API 概览

| 方法 | 路由 | 说明 |
| :--- | :--- | :--- |
| `POST` | `/api/v1/auth/login` | 管理员登录认证并获取 JWT Token |
| `GET` | `/api/v1/status` | 运行状态、版本号、缓存条数及端口信息 |
| `GET` | `/api/v1/metrics` | 吞吐量、QPS、命中率、拦截率与 24h 时序指标 |
| `GET` | `/api/v1/querylog` | 分页查询日志，支持域名模糊搜、状态过滤 |
| `POST` | `/api/v1/querylog/clear`| 清空日志流水并强制 VACUUM 释放磁盘物理空间 |
| `POST` | `/api/v1/tools/query` | 原生全流程 DNS 诊断工具接口 (支持 A/AAAA/CNAME/MX/TXT) |
| `GET` | `/api/v1/pipeline` | 获取流水线节点状态拓扑 |
| `POST` | `/api/v1/pipeline/toggle` | 就地启用/停用指定节点（如动态分流调度引擎） |
| `GET` | `/api/v1/settings` | 读取系统核心配置文件 |
| `PUT` | `/api/v1/settings` | 热更新系统设置并自动保存与重载生效 |
| `POST` | `/api/v1/cache/flush` | 清空当前运行时内存 DNS 缓存 |
| `GET` | `/api/v1/sse/querylog` | 实时查询日志 Server-Sent Events (SSE) 流 |

---

## 许可证

NexDNS 遵循 [MIT License](LICENSE) 开源协议。
