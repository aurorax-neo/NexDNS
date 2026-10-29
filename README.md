# NexDNS: 高性能融合型原生 DNS 服务

NexDNS 是一个自研原生统一单内核的下一代高性能 DNS 综合解决方案，集成了现代化 UI 工作台看板、多引擎规则安全拦截、毫秒级日志全链路诊断、高度灵活的流水线智能分流、GeoIP 防污染校验、多协议并发上游池与 TCP 端口建连测速优选于单一进程。

基于 **Rust 异步高性能单内核 + React 19 现代工作台** 构建，彻底告别传统多容器、多进程套娃（53 -> 4053 -> 5053 -> 6053）所带来的跨进程开销与网络跳数延迟。

---

## 核心特性

1. **统一自研原生单内核**：
   - 零跨进程转发与网络跳数损耗，内存零拷贝流水线执行。
   - 客户端来源 IP 与全流程耗时纳秒级贯穿，日志记录 100% 真实。
   - 单二进制交付，前端 SPA 静态资源由 Rust 内部原生内嵌服务。

2. **流水线插件化分流 (Pipeline Routing)**：
   - Qtype 65 快捷拦截，防止客户端绕过 DNS 分流。
   - 域名分类规则（Direct 大陆直连、Proxy 境外代理、Private 私有网络）。
   - **防 DNS 污染检测**：大陆上游返回非中国 IP 时判定为污染，立即丢弃并自动回退（Fallback）至纯净境外代理组。

3. **高效威胁与安全规则拦截 (Security Filtering)**：
   - 兼容 Adblock Plus (ABP) 规则语法（`||domain^`、`@@||whitelist^`）、Hosts 格式及正则表达式。
   - 支持 ZeroIP (`0.0.0.0` / `::`)、NXDOMAIN 与 REFUSED 阻断模式。
   - 内置域名规则命中实时诊断工具。

4. **并发查询与最优测速 (Fastest IP & SmartCache)**：
   - 支持 UDP 53、TCP 53 以及加密 DNS over HTTPS (DoH)。
   - **TCP 80/443 建连测速优选**：针对上游返回的多个 IP 记录并发发起非阻塞握手，将最低 RTT 的最快 IP 置顶返回客户端。
   - **智能缓存**：支持 `Serve-Expired`（过期记录秒返 + 后台静默刷新）与高频域名自动预取。

5. **现代专业工作台 (React 19)**：
   - 严格遵循 `/Users/wunk/code/前端设计规范总结` 规范。
   - 表面层级细分（1%~3% 明度差，深色模式字重微调防膨胀）。
   - 数字与时延防抖（`font-variant-numeric: tabular-nums`）。
   - 规范化浮层：Modal（400/520/720px 三档）、Drawer（深度诊断报文）、Popconfirm（行内操作）、AlertDialog（高危清空）、Snackbar（可撤销删除）。
   - URL 驱动的无抖动 Tab 状态持久化。

---

## 快速开始

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

### 2. 开发模式

```bash
# 终端 1: 启动后端 (指定测试端口 15353 与 API 端口 3000)
cd backend
cargo run -- --dns-port 15353 --web-port 3000

# 终端 2: 启动前端热重载 Vite 服务 (端口 5173，自动反代 /api 至 3000)
cd frontend
pnpm dev
```

打开浏览器访问 `http://localhost:5173` 或生产端口 `http://localhost:3000` 即可使用。
