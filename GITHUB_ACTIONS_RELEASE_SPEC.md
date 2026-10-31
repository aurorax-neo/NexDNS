# GitHub Actions 多架构发布与 Docker 构建工程规范

> 版本：v2.0（含发布日志约束）
> 适用范围：Rust / Go / Node 等多语言项目的多架构制品发布与容器镜像构建

---

## 目录

- [一、核心架构设计原则](#一核心架构设计原则)
  - [原则一：绝对禁止在 Docker 内通过 QEMU 软件模拟编译](#原则一绝对禁止在-docker-内通过-qemu-软件模拟编译)
  - [原则二：CI 运行环境与本地网络镜像严格解耦](#原则二ci-运行环境与本地网络镜像严格解耦)
  - [原则三：严格流水线单向依赖与制品解耦](#原则三严格流水线单向依赖与制品解耦)
  - [原则四：发布日志与制品强一致](#原则四发布日志与制品强一致)
- [二、标准容器文件规范（双模模式）](#二标准容器文件规范双模模式)
- [三、发布日志与版本管理规范](#三发布日志与版本管理规范)
- [四、标准化 GitHub Actions 工作流模板](#四标准化-github-actions-工作流模板)
- [五、新项目接入检查清单（Checklist）](#五新项目接入检查清单checklist)
- [六、常见故障与排错指南](#六常见故障与排错指南)

---

## 一、核心架构设计原则

### 原则一：绝对禁止在 Docker 内通过 QEMU 软件模拟编译

- **反模式**：在 x86_64 构建机上使用 `docker buildx` + QEMU 模拟 aarch64 (ARM64) 执行源码编译（如 `cargo build --release`、`go build`、`cmake/make`）。QEMU 在用户态模拟指令集的 CPU 开销极大，编译复杂依赖时耗时膨胀 15~20 倍（常需 1.5~3 小时），且极易触发内存耗尽或超时挂起。
- **标准方案**：**「交叉编译二进制在前，纯拷贝装配在后」**。在构建机裸机环境利用专用工具链（Rust `cross` / Go `GOARCH=arm64`）在 2 分钟内编译好原生静态二进制；Docker 打包阶段仅执行文件拷贝（COPY），多架构打包与推送控制在 30 秒内。

### 原则二：CI 运行环境与本地网络镜像严格解耦

- **反模式**：在 `Dockerfile` 或源码配置文件中硬编码国内镜像源（如 `rsproxy.cn`、国内 npm 镜像源）。
- **标准方案**：GitHub Actions 运行节点位于微软 Azure 海外机房（美西/西欧），反向连接国内镜像源易遭遇高丢包率、TLS 握手超时或网络挂起。流水线必须在前置阶段剥离或替换本地私有镜像源，直接走官方源拉取依赖。

### 原则三：严格流水线单向依赖与制品解耦

- 采用规范阶段依赖：`build-frontend` -> `build-binaries` -> [`publish-release`, `docker`]。
- Docker 打包任务强依赖 `build-binaries` 的编译产物，杜绝同一套代码在不同 Job 中重复编译。

### 原则四：发布日志与制品强一致

> **每一次对外发布必须同时满足「SemVer Tag + CHANGELOG 版本条目 + 制品校验和 + 镜像 digest」四要素，任一缺失则流水线失败，禁止静默发布。**

拆成三条硬约束：

1. **Tag 必须是 SemVer**：`vMAJOR.MINOR.PATCH[-pre]`，与 `Cargo.toml` / `package.json` 的 `version` 保持一致。
2. **CHANGELOG 必须有当前版本段落**：发布 job 前置校验，找不到版本标题直接 fail，不允许「先发后补」。
3. **Release Notes 必须非空且带分类**：Breaking Change 必须独立成段，禁止把所有变更混成一段。

---

## 二、标准容器文件规范（双模模式）

项目中维护两个 Dockerfile，严格分离「自动化发布打包」与「本地单机开发构建」：

### 1. 发布专用多架构模板（`Dockerfile.release`）

利用 Docker Buildx 原生内置的 `TARGETARCH` 变量（自动匹配 `amd64` / `arm64`），仅执行静态文件注入，容器内无任何编译工具链，镜像体积最小、构建速度最快。

```dockerfile
FROM alpine:latest
ARG TARGETARCH
WORKDIR /app

RUN apk add --no-cache ca-certificates tzdata && \
    ln -snf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone
ENV TZ=Asia/Shanghai

# 从构建机准备好的目录拷贝对应架构的纯静态单二进制
COPY docker-bin/${TARGETARCH}/<二进制名称> /app/<二进制名称>
RUN chmod +x /app/<二进制名称>

EXPOSE <服务端口>
ENTRYPOINT ["/app/<二进制名称>"]
CMD ["--config", "/app/data/config.yaml"]
```

### 2. 本地开发兼容构建模板（`Dockerfile`）

供本地开发者无需依赖外部 CI 时单机执行 `docker compose up --build`，不写死任何特定地域的网络代理。

```dockerfile
ARG BASE_IMAGE=alpine:latest
ARG BUILDER_IMAGE=rust:alpine

FROM ${BUILDER_IMAGE} AS builder
RUN apk add --no-cache musl-dev gcc
WORKDIR /app
COPY Cargo.toml ./
COPY backend/ backend/
WORKDIR /app/backend
RUN cargo build --release

FROM ${BASE_IMAGE}
WORKDIR /app
RUN apk add --no-cache ca-certificates tzdata && \
    ln -snf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone
ENV TZ=Asia/Shanghai
COPY --from=builder /app/target/release/<二进制名称> /app/<二进制名称>
EXPOSE <服务端口>
ENTRYPOINT ["/app/<二进制名称>"]
CMD ["--config", "/app/data/config.yaml"]
```

---

## 三、发布日志与版本管理规范

### 1. `CHANGELOG.md`（Keep a Changelog 格式）

```markdown
# Changelog

本项目所有重要变更均记录于此文件。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.2] - 2026-06-01

### 💥 Breaking Changes
- 配置项 `listen.addr` 拆分为 `listen.host` + `listen.port` (#128)

### 🚀 Features
- 支持多架构镜像发布 (#131)

### 🐛 Bug Fixes
- 修复 aarch64 下 musl 静态链接缺失 ca-certificates (#134)

### 🧰 Maintenance
- CI 剥离本地 cargo 镜像源配置 (#135)

## [0.1.1] - 2026-05-20
...
```

> **约定**：`## [x.y.z]` 这一行是流水线提取 release body 的锚点，**标题格式必须严格一致**，否则校验脚本会失败。

### 2. `.github/release.yml`（GitHub 自动生成 notes 的分类规则）

即使最终以 CHANGELOG 为准，也建议保留该文件，让 GitHub 在 Web 端「Generate release notes」时按业务分类，而不是平铺 PR 标题。

```yaml
changelog:
  exclude:
    labels:
      - ignore-for-release
    authors:
      - dependabot
      - github-actions
  categories:
    - title: 💥 Breaking Changes
      labels: [breaking-change, semver-major]
    - title: 🚀 Features
      labels: [feature, enhancement]
    - title: 🐛 Bug Fixes
      labels: [bug, fix]
    - title: 🧰 Maintenance
      labels: [chore, refactor, ci, docs]
    - title: 📦 Dependencies
      labels: [dependencies]
    - title: Other Changes
      labels: ["*"]
```

### 3. 版本号三处一致性要求

| 位置 | 示例 | 校验方式 |
|---|---|---|
| Git Tag | `v0.1.2` | 正则校验 SemVer |
| 源码版本号（`Cargo.toml` / `package.json`） | `0.1.2` | 与 Tag 去 `v` 前缀后比对 |
| `CHANGELOG.md` 标题 | `## [0.1.2] - 2026-06-01` | grep 版本号锚点 |

任一不一致，`publish-release` job 直接 fail。

### 4. 可选：全自动 CHANGELOG（git-cliff）

若不想手写 CHANGELOG，可用 `git-cliff` 在 `publish-release` 前置生成段落，配合 Conventional Commits：

```yaml
      - name: Generate changelog section
        uses: orhun/git-cliff-action@v4
        with:
          config: cliff.toml
          args: --latest --strip header
        env:
          OUTPUT: CHANGELOG.section.md
```

再让 `Extract release body` 读取该文件即可。此时 `Verify CHANGELOG entry` 改为校验生成文件非空。

> **但无论如何，「发布必须有日志」这条约束本身不能被省略。**

---

## 四、标准化 GitHub Actions 工作流模板

文件路径：`.github/workflows/release.yml`

```yaml
name: Release & Publish

on:
  push:
    tags:
      - "v*"
  workflow_dispatch:
    inputs:
      tag:
        description: '发布版本号 (例如 v0.1.2)'
        required: false
        default: 'v0.1.2'

permissions:
  contents: write
  packages: write

# 关键：开启并发控制，重推 Tag 或修复时自动取消前序挂起任务，杜绝 Actions 额度浪费
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # 阶段 1: 前端构建 (无前端项目可省略此阶段)
  build-frontend:
    name: Build Frontend UI
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: latest

      - name: Install dependencies
        working-directory: frontend
        run: pnpm install

      - name: Build frontend
        working-directory: frontend
        run: pnpm build

      - name: Upload frontend artifact
        uses: actions/upload-artifact@v4
        with:
          name: frontend-dist
          path: frontend/dist/
          retention-days: 1

  # 阶段 2: 多架构二进制并行交叉编译 (核心)
  build-binaries:
    name: Build Binary (${{ matrix.target }})
    needs: build-frontend
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
            use_cross: false
          - os: ubuntu-latest
            target: aarch64-unknown-linux-musl
            use_cross: true
          - os: macos-latest
            target: x86_64-apple-darwin
            use_cross: false
          - os: macos-latest
            target: aarch64-apple-darwin
            use_cross: false
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            use_cross: false
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      # 规范：剥离本地特有的国内代理配置，防止海外 Runner 访问超时
      - name: Remove local package proxy in CI
        shell: bash
        run: rm -f backend/.cargo/config.toml

      - name: Download frontend artifact
        uses: actions/download-artifact@v4
        with:
          name: frontend-dist
          path: frontend/dist

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install musl tools on Linux
        if: matrix.os == 'ubuntu-latest' && !matrix.use_cross
        run: sudo apt-get update && sudo apt-get install -y musl-tools musl-dev

      - name: Install Cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build target binary
        shell: bash
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Package archive
        shell: bash
        run: |
          TAG_NAME="${{ inputs.tag || github.ref_name }}"
          [ "$TAG_NAME" = "" ] || [ "$TAG_NAME" = "main" ] && TAG_NAME="dev"
          PKG_NAME="<项目名>_${TAG_NAME}_${{ matrix.target }}"
          ROOT_DIR="$(pwd)"
          mkdir -p "${ROOT_DIR}/release-packages" release-pkg

          BIN_NAME="<二进制名称>"
          EXT=""
          if [ "${{ runner.os }}" = "Windows" ]; then
            EXT=".exe"
          fi

          cp "target/${{ matrix.target }}/release/${BIN_NAME}${EXT}" "release-pkg/${BIN_NAME}${EXT}"
          cp README.md release-pkg/ 2>/dev/null || true
          cp LICENSE release-pkg/ 2>/dev/null || true

          if [ "${{ runner.os }}" = "Windows" ]; then
            cd release-pkg
            7z a "${ROOT_DIR}/release-packages/${PKG_NAME}.zip" *
          else
            cd release-pkg
            tar -czvf "${ROOT_DIR}/release-packages/${PKG_NAME}.tar.gz" *
          fi

      - name: Upload package artifact
        uses: actions/upload-artifact@v4
        with:
          name: package-${{ matrix.target }}
          path: release-packages/*
          retention-days: 1

  # 阶段 3: 发布 GitHub Release 静态包与校验和（含发布日志强校验）
  publish-release:
    name: Publish GitHub Release Assets
    needs: build-binaries
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      # ── 约束 1：Tag 必须符合 SemVer ──────────────────────────
      - name: Validate tag format (SemVer)
        shell: bash
        run: |
          TAG="${{ inputs.tag || github.ref_name }}"
          if ! [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
            echo "::error::Tag '$TAG' 不符合 SemVer（应为 vX.Y.Z 或 vX.Y.Z-pre）"
            exit 1
          fi

      # ── 约束 2：源码版本号与 Tag 一致（按语言二选一）─────────
      - name: Verify version matches Tag
        shell: bash
        run: |
          TAG="${{ inputs.tag || github.ref_name }}"; VER="${TAG#v}"
          SRC_VER=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
          # Node 项目改为： SRC_VER=$(node -p "require('./package.json').version")
          if [ "$SRC_VER" != "$VER" ]; then
            echo "::error::Cargo.toml version=$SRC_VER 与 Tag=$VER 不一致"
            exit 1
          fi

      # ── 约束 3：CHANGELOG 必须包含当前版本段落且非空 ─────────
      - name: Verify CHANGELOG entry
        shell: bash
        run: |
          TAG="${{ inputs.tag || github.ref_name }}"; VER="${TAG#v}"
          if ! grep -qE "^##\s*\[?${VER}\]?" CHANGELOG.md; then
            echo "::error::CHANGELOG.md 缺少版本 $VER 的条目，请先补充后重推 Tag"
            exit 1
          fi

      - name: Extract release body from CHANGELOG
        id: changelog
        shell: bash
        run: |
          TAG="${{ inputs.tag || github.ref_name }}"; VER="${TAG#v}"
          BODY=$(awk -v ver="$VER" '
            $0 ~ "^## \\[?"ver"\\]?" {flag=1; next}
            flag && /^## / {exit}
            flag && NF {print}
          ' CHANGELOG.md)
          if [ -z "${BODY//[[:space:]]/}" ]; then
            echo "::error::CHANGELOG 中版本 $VER 内容为空"
            exit 1
          fi
          { echo "body<<__EOF__"; echo "$BODY"; echo "__EOF__"; } >> "$GITHUB_OUTPUT"

      - name: Download all package artifacts
        uses: actions/download-artifact@v4
        with:
          path: downloaded-artifacts
          pattern: package-*
          merge-multiple: true

      - name: Generate Checksums
        shell: bash
        run: |
          mkdir -p release-assets
          cp downloaded-artifacts/* release-assets/
          cd release-assets && sha256sum * > checksums.txt

      - name: Upload assets to GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: release-assets/*
          tag_name: ${{ inputs.tag || github.ref_name }}
          # 显式提供 body 时，GitHub 不会再自动生成 notes
          body: ${{ steps.changelog.outputs.body }}
          generate_release_notes: false
          draft: false
          prerelease: ${{ contains(github.ref_name, '-') }}

  # 阶段 4: 秒级发布多架构 Docker 镜像 (复用已编译产物)
  docker:
    name: Publish Multi-arch Docker Image to GHCR
    needs: build-binaries
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Download linux-amd64 binary artifact
        uses: actions/download-artifact@v4
        with:
          name: package-x86_64-unknown-linux-musl
          path: pkg-amd64

      - name: Download linux-arm64 binary artifact
        uses: actions/download-artifact@v4
        with:
          name: package-aarch64-unknown-linux-musl
          path: pkg-arm64

      - name: Prepare multi-arch binaries
        shell: bash
        run: |
          mkdir -p docker-bin/amd64 docker-bin/arm64
          tar -xvf pkg-amd64/*.tar.gz -C docker-bin/amd64/
          tar -xvf pkg-arm64/*.tar.gz -C docker-bin/arm64/
          chmod +x docker-bin/amd64/<二进制名称> docker-bin/arm64/<二进制名称>
          ls -la docker-bin/amd64/ docker-bin/arm64/

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GitHub Container Registry (GHCR)
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ghcr.io/${{ github.repository_owner }}/<镜像名称>
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=raw,value=latest,enable=${{ github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/v') }}

      - name: Build and push multi-arch image
        uses: docker/build-push-action@v6
        with:
          context: .
          file: Dockerfile.release
          platforms: linux/amd64,linux/arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      # 记录镜像 digest 到 Job Summary，保证发布可追溯
      - name: Record image digest
        shell: bash
        run: |
          IMAGE="ghcr.io/${{ github.repository_owner }}/<镜像名称>"
          TAG="${{ inputs.tag || github.ref_name }}"
          DIGEST=$(docker buildx imagetools inspect "${IMAGE}:${TAG#v}" --format '{{json .Manifest}}' \
                   | jq -r '.digest' 2>/dev/null || echo "unknown")
          {
            echo "### 📦 Multi-arch image published"
            echo "- Image: \`${IMAGE}:${TAG#v}\`"
            echo "- Digest: \`${DIGEST}\`"
            echo "- Platforms: linux/amd64, linux/arm64"
          } >> "$GITHUB_STEP_SUMMARY"
```

> **可选**：若希望镜像 digest 也进入 Release Notes，可将 `publish-release` 改为 `needs: [build-binaries, docker]`，用 artifact 传递 digest——代价是两 job 串行，按需取舍。

---

## 五、新项目接入检查清单（Checklist）

### 1. 替换占位符

- `<项目名>`：如 `nexdns`、`blockyx`
- `<二进制名称>`：编译生成的 ELF / 可执行文件名称
- `<镜像名称>`：GHCR 发布的镜像路径（全小写）
- `<服务端口>`：服务监听的主机映射端口

### 2. 忽略中间目录

在 `.gitignore` 中追加：

```
docker-bin/
release-packages/
```

### 3. 保留并发取消配置

务必配置 `concurrency: cancel-in-progress: true`，杜绝重复推送 Tag 时前序任务持续耗尽免费构建额度。

### 4. 统一采用 musl 纯静态链接

Linux 构建必须指定 `*-unknown-linux-musl`，避免目标 Alpine 镜像由于缺乏 glibc 产生 `file not found` 无法执行的问题。

### 5. 维护 `CHANGELOG.md`

每次发版前手写 `## [x.y.z]` 段落；或接入 `git-cliff` / `release-please` 自动生成（二者任选一，禁止「完全依赖 GitHub 自动 notes」）。

### 6. 填写 `.github/release.yml`

保证 Web 端自动生成 notes 时带正确分类与 breaking change 标识。

### 7. 版本号三处一致

`Cargo.toml`/`package.json` 的 `version`、Git Tag、`CHANGELOG.md` 标题三者必须相同，流水线已做校验。

### 8. 预发布用 `-`

如 `v0.2.0-rc.1`，`prerelease` 自动置 true，避免污染 `latest` 标签。

---

## 六、常见故障与排错指南

| 现象 | 根因分析 | 解决措施 |
|---|---|---|
| **Docker 构建持续数小时不结束** | 使用 QEMU 在 x86 上模拟编译 ARM64 源码 | 采用本规范的 `Dockerfile.release`，直接解压复制宿主机编译好的静态二进制，耗时降至 30 秒内。 |
| **Cargo / npm 拉取依赖握手超时** | 配置文件写死了国内镜像源（如 rsproxy/npmmirror） | 在构建脚本开头执行 `rm -f .cargo/config.toml`，确保 CI 环境直连官方源。 |
| **容器启动报 `exec: not found`** | 编译时使用了默认 glibc (gnu)，而运行基础镜像为 Alpine (musl) | Linux 编译目标统一切换至 `x86_64-unknown-linux-musl` 与 `aarch64-unknown-linux-musl`。 |
| **反复修改 Tag 触发多个 Run 浪费配额** | 工作流缺少并发控制组 | 在根级别声明 `concurrency: { group: ..., cancel-in-progress: true }`。 |
| **Release 已发布但 Notes 为空** | 仅用了 `generate_release_notes: true`，但对应 PR 没打 label 或全被 exclude | 补 `.github/release.yml` 的 `categories` 兜底项 `"*"`，并改为显式 `body` 来自 CHANGELOG。 |
| **Tag 与 Cargo.toml 版本不一致却发布成功** | 缺少版本一致性校验 | 使用 `Verify version matches Tag` 步骤，失败即 fail。 |
| **CHANGELOG 忘记写当前版本，Release 内容错位到上一版本** | `generate_release_notes` 自动生成不读 CHANGELOG | 启用 `Verify CHANGELOG entry` 前置校验 + `awk` 精确按版本标题截取 body。 |
| **镜像发布成功但无法追溯是哪个 commit/digest** | Docker job 未输出摘要 | 增加 `Record image digest` 写入 `$GITHUB_STEP_SUMMARY`。 |

---

## 七、总结

本规范从「**制品怎么产出**」到「**制品凭什么可发布**」形成闭环：

- **原则一~三** 解决构建效率与制品解耦（交叉编译 + 纯拷贝装配 + 单向依赖）。
- **原则四** 解决发布可追溯性（Tag / 版本号 / CHANGELOG / 校验和 / 镜像 digest 五者环环相扣）。
- **五者缺一即阻断，禁止静默发布。**
