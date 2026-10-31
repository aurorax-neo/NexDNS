# Changelog

本项目所有重要变更均记录于此文件。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.4] - 2026-10-31

### 🚀 Features
- **客户端网络来源智能识别**：活跃客户端 IP 榜单移除硬编码的「内网来源」字样，改为对每个客户端 IP 实时识别并标注「内网」或「公网」徽标（覆盖 IPv4 私网/回环/链路本地/CGNAT 100.64/10、IPv6 ULA/链路本地/回环及 IPv4-mapped IPv6 地址）。
- **DNS 缓存状态指标语义清晰化**：缓存状态面板表头重命名为「缓存查询次数」，并补充悬停口径说明。

### 🐛 Bug Fixes
- **高频查询域名榜单剔除拦截流量**：`top_domains` 统计不再累加 `Blocked` 状态的请求，广告/追踪域名仅归入拦截榜单，不再污染正常高频解析榜单。
- **缓存命中率统计口径修正**：私有域名拒绝、规则拦截与本地 Hosts 重写均在进入缓存查询前短路返回，现已从缓存统计分母中剔除（不再计为缓存未命中），真实反映缓存层命中效率。
- **实时查询日志流可靠关闭**：修复快速点击切换时的闭包陈旧状态（stale closure）、筛选/翻页触发 SSE 重建竞态、读取循环未及时响应 `abort` 以及异常断开未回滚按钮状态导致的偶发无法关闭问题。

### 🧰 Maintenance
- 补充缓存命中率口径单元测试（`test_cache_hit_rate_only_counts_cache_path_queries`）。


## [0.1.3] - 2026-10-31

### 🚀 Features
- **上游分组独立整体测速**：新增 `POST /api/v1/upstreams/test-group` 接口，并在前端每个上游分组（引导 DNS、大陆直连组、GFW 组、海外备用组）卡片头部提供独立的「整体测速」按钮。
- **全量与分组测速并发化**：后端将上游节点连通性探测从串行改为 `tokio::spawn` 并发执行，20+ 节点全量测速耗时从 10s+ 缩短至 1~2s。

### 🐛 Bug Fixes
- **不同分组相同上游测速状态彻底隔离**：修复前端测速状态与延迟结果仅按 `addr` 匹配导致跨组同地址节点（如 GFW 组与海外备用组同时配置 `1.1.1.1`）联动刷新的问题，统一改用 `${group}:${addr}` 复合键隔离。


## [0.1.2] - 2026-10-31

### 🚀 Features
- **Argon2id 密码哈希与平滑升级**：管理员密码升级为标准 `Argon2id` 算法存储，兼容旧版 SHA-256 密码在首次成功登录时自动无感升级至 Argon2id，并新增内存级 `SESSION_CACHE` 与防暴力破解限流。
- **双栈监听与动态拦截配置**：`DnsServer` 支持 `ipv4` / `ipv6` / `dual` 双栈独立监听（开启 `IPV6_V6ONLY`）；`AdblockPlugin` 支持运行时热更新阻断模式（`ZeroIp` / `NxDomain` / `Refused`）与自定义阻断 TTL。
- **热点域名静默预取（Prefetch-Domain）**：当缓存条目剩余 TTL 低于 20% 时，后台异步触发上游回源刷新，消除热点域名过期瞬间的查询延迟毛刺。

### 🐛 Bug Fixes
- **修复修改密码 SQL 参数下标越界**：修复 `AuthManager::change_password` 中 `WHERE username = ?3` 的占位符下标错误，恢复管理员密码修改功能。
- **消除认证 Token 在 URL 中的暴露**：实时日志 SSE 流与 CSV 导出均改为通过 `Authorization: Bearer` 请求头鉴权 + `ReadableStream` / `Blob` 处理，避免 Token 出现在 URL 与访问日志中。
- **规则文件防路径穿越与配置安全校验**：规则文件读写接口增加严格文件名白名单校验；原始配置更新与设置保存接入 `validate_config_safety` 强校验并实时热重载运行时状态。
- **拦截规则删除支持完整正文恢复**：删除拦截规则前预拉取规则正文快照，点击 Snackbar「撤销」可完整还原规则内容。

### 🧰 Maintenance
- **数据库连接池统一收敛**：`RuleFileManager` 复用主 `Database` 连接池，消除多连接池并发写入 SQLite WAL 的锁竞争；DNS TCP 监听增加 512 并发信号量与 5 秒读取超时保护；上游并发查询首轮限制最多 4 节点防止连接风暴。
- **Docker 多架构构建极速化**：新增 `Dockerfile.release`，CI 直接复用 `build-binaries` 阶段产出的 `musl` 静态二进制装配镜像，彻底消除 QEMU 模拟编译耗时。


## [0.1.1] - 2026-10-30

### 💥 Breaking Changes
- **移除 `Qtype 65`（HTTPS RR）阻断与 `Serve-Expired` 过期宽限机制**：完全放行标准 IETF RFC 9460 HTTPS 资源记录以保障现代浏览器 HTTP/3 与 ECH 协商；缓存生命周期回归标准 DNS TTL 语义（默认最小 60s、最大 120s）。

### 🚀 Features
- **智能分流调度引擎整合与优雅降级保底**：将分流与防污染回退整合为统一的「智能分流调度引擎」阶段，开启前强校验 GFW 与海外备用组配置；当境外上游超时或代理不可达时自动回退至大陆有效公网 IP 应答，防止返回 `SERVFAIL` 造成断网。
- **全场景一键清空缓存**：在系统设置、DNS 缓存状态面板与流水线节点抽屉中均提供即时清空内存缓存入口。

### 🐛 Bug Fixes
- **流水线拓扑开关持久化**：修复流水线节点启停状态在页面切换或自动保存时被覆盖的问题，状态变更即时写回 `config.yaml`。
- **移除冗余设置卡片**：清理系统设置页中与流水线拓扑重复的配置项。


## [0.1.0] - 2026-10-30

### 🚀 Features
- **自研原生统一单内核架构**：基于 Rust + Tokio + Axum + React 19 构建，单二进制内嵌前端静态产物，零外部代理进程依赖。
- **端口物理隔离**：标准 DNS（53/UDP `SO_REUSEPORT` 多队列 + 53/TCP）、独立 RFC 8484 DoH 服务（8053/TCP）与 Web 管理控制台（3000/TCP）严格隔离。
- **全链路插件化流水线**：内置本地 Hosts/CNAME 重写、兼容 AdGuard/ABP/Hosts/Regex 的高速拦截引擎、智能内存缓存、基于 GeoSite/GeoIP 的国内外分流与防污染引擎、多协议并发上游（UDP/TCP/DoT/DoH + SOCKS5 代理）以及 TCP 80/443 握手测速优选（严格保持 CNAME 位于 A/AAAA 之前）。
- **RFC 6891 / RFC 7873 EDNS Cookie 对齐**：精准回显或剥离 EDNS Cookie，彻底消除下游 `Client COOKIE mismatch` 警告。
- **可观测性与控制台**：内置 DNS 在线诊断工具（Query Tool）、24 小时趋势图、SmartDNS 风格缓存状态表、SQLite WAL 异步批量日志落盘与分钟级自动滚动清理。
