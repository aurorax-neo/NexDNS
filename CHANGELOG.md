# Changelog

本项目所有重要变更均记录于此文件。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.7] - 2026-11-06

### 🚀 Features
- **实时查询日志全宽布局与视觉防闪烁优化**：移除查询日志页「当前页分布概览」与右侧常驻分栏卡片，主表格 100% 铺满整屏宽度并重平衡 8 列百分比配比与内边距对齐；保留点击行滑出详情抽屉；收敛「查询超时」配置入口至「系统设置」页；根除实时跟踪流（SSE）刷新时的滚动条与行背景闪白条问题。
- **RFC 标准本地特殊域与反向解析域完整支持（RFC 6303 / 6761 / 6762 / 6763 / 7050 / 9462）**：对 `localhost` / `*.localhost` 前置权威返回回环地址，对 `127.0.0.1` / `::1` 的 `PTR` 反向查询权威返回 `localhost.`；补齐 `0.in-addr.arpa`、`c.f.ip6.arpa`（`fc00::/8`）等缺失的私网反向解析区，将 `.test`、`.invalid`、`.local`、`.home.arpa`、`.internal`、`.lan`、`.onion`、单标签主机名及 `._dns-sd._udp.` 局域网服务发现全部纳入本地权威响应，严禁外发公网。

### 🐛 Bug Fixes
- **修复 `domain_private` 返回 `Query Refused` 引发终端每秒重试风暴及重写失效**：将「自定义重写」提升至 `domain_private` 之前执行；区分 `private.txt` 中的应用回环辅助域（如 `localhost.weixin.qq.com` 走大陆直连解析）与纯内网/路由器发现域（如 `tplinkac.tplogin.cn` 及私网 `PTR`），对未配置重写的内网域改为返回标准 `NXDOMAIN` 并写入 60s 否定缓存，彻底消除红色误拦截告警与 AP 每秒死循环重试。
- **补齐本地否定应答 Authority `SOA` 记录（RFC 2308 §3 & §5）**：为所有本地合成的 `NXDOMAIN` 与 `NODATA` 应答自动注入权威 `SOA` 记录（`TTL = 300s`），使下游操作系统与 AP 解析器能够按标准缓存否定结果。
- **修复缓存命中时 `CNAME` 覆盖真实解析 IP 显示**：日志提取逻辑改为优先扫描应答区中的 `A` / `AAAA` 真实 IP 记录，仅在无 IP 记录时回退显示 `CNAME` / `PTR`。
- **TCP 测速改为非阻塞后台探测**：冷启动未命中测速缓存时直接极速返回 DNS 应答并在后台异步探测前 3 个候选 IP，消除容器无公网 IPv6 出口或目标不通时阻塞 `AAAA` / `A` 查询 200ms+ 的问题。

## [0.1.6] - 2026-11-06

### 🚀 Features
- **配置与数据存储架构解耦**：管理员账号与 Argon2id 密码哈希、DNS 重写规则、拦截规则元数据及分流规则配置统一收敛持久化至 `config.yaml`（拦截规则正文独立存储于 `data/rules/`），SQLite 数据库（`nexdns.db`）仅保留 `query_logs` 查询日志表，并在启动时自动完成旧表平滑迁移与空间回收。
- **磁盘空间水位监测与防撑爆自动清理**：基于 `statvfs` 实时监测数据盘使用率、剩余可用空间与数据库体积（支持 `max_disk_usage_percent`、`min_free_disk_mb`、`max_db_size_mb` 配置），每 15 秒后台巡检并在超水位时分批淘汰最旧日志、执行 `PRAGMA wal_checkpoint(TRUNCATE)` 与 `VACUUM` 释放物理磁盘空间。
- **上游超时热更新入口**：新增 `POST /api/v1/upstreams/timeout` 接口，并在「上游配置」与「系统设置」页面提供毫秒级超时实时调节控件，支持无重启热生效。
- **实时查询日志工作台紧凑双栏重构**：合并顶部在线诊断栏、筛选工具栏与磁盘状态指示器，表格支持行内一键复制/拦截/放行，并在宽屏下提供主从分栏详情检视器，消除页面大面积留白。

### 🐛 Bug Fixes
- **修复双栈模式下 DNS TCP `[::]:53` 绑定冲突 (`Address in use, os error 98`)**：`DnsServer` 的 TCP 监听改用 `socket2` 并在 IPv6 地址上显式启用 `IPV6_V6ONLY`（`set_only_v6(true)`），解决 Linux 默认 `net.ipv6.bindv6only=0` 下先绑定 `0.0.0.0:53` 后再绑定 `[::]:53` 触发端口占用告警导致 IPv6 TCP 53 未监听的问题。
- **修复跨楼层/跨 VLAN 大包分片丢弃导致的解析卡顿**：遵循 RFC 8900 / DNS Flag Day 2020 标准，将 UDP 响应载荷限制在安全 MTU（EDNS `1232B` / 传统 `512B`）以内；超长时优先剥离非必要附加记录，仍超长则自动标记 `TC=1`（Truncated）引导客户端立即回退 TCP 53，避免三层交换机静默丢弃 UDP 分片造成 2~5 秒超时。
- **修复 `ipv4_only` / `ipv6_only` 缓存命中漏过滤与双栈退避卡顿**：在单栈响应模式下对异构地址族查询（如 `ipv4_only` 下的 `AAAA`）前置短路返回空 `NoError`（NODATA），并在写缓存前与读缓存命中时双重执行地址族过滤，彻底杜绝无 IPv6 出口楼层收到 `AAAA` 记录。
- **修复未启用代理时境外分流连环超时与 `ServFail` 断网**：`domain_proxy` 规则在 `gfw` 与 `overseas` 组均不可达时自动降级至 `mainland` 大陆上游兜底；上游节点失败 2 次即进入阶梯式熔断冷却（10s/30s/60s），全组冷却期间仅单节点短超时探测并跳过溢出批次，消除 4~8 秒阻塞。
- **修复旧日志无法定时删除与 `retention_days` 遗留字段**：自动将旧版 `retention_days` 迁移为 `retention_mins` 并在序列化时剥离；修复 ISO-8601 时间戳跨时区 SQL 比较条件，确保定时清理稳定生效。
- **TCP 测速改为 80/443 并发竞速**：使用 `select_ok` 并发探测 80 与 443 端口，消除目标仅开放 443 时在 80 端口上的 200ms 串行超时等待。

## [0.1.5] - 2026-11-05

### 🚀 Features
- **浅色主题视觉层级深度优化**：消除浅色模式下生硬突兀的暗色块。DNS 缓存状态表格表头移除深灰蓝硬编码，改为中性浅灰底色；拦截规则与分流规则抽屉内的规则查看器和文本编辑框由写死的纯黑背景重构为深浅双模式自然自适应；分段控制器组件支持明亮的中性层级。

### 🐛 Bug Fixes
- **日志表格超长 IPv6 越界重叠彻底根除**：将查询日志主表格最小宽度提升至 1080px，客户端 IP 列由 115px 扩宽至 180px；单元格部署 `overflow-hidden` 与 `truncate block max-w-full` 约束并保留完整浮窗提示，彻底解决 35+ 字符长 IPv6 地址碰撞覆盖相邻规则列文字的显示问题。
- **分流规则与响应结果溢出截断保护**：对超长规则名和超长 CNAME 应答增加自适应保护，保证各列在所有屏幕宽度下边界稳定不挤压。

### 🧰 Maintenance
- **容器网络架构规范化**：默认 Compose 编排显式声明 `network_mode: bridge`，直连 Docker 宿主机原生默认网桥，避免在宿主机创建多余自定义网桥。

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
