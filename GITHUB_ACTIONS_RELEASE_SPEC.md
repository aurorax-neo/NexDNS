# GitHub Actions 多架构发布与 Docker 构建工程规范

## 一、核心架构三大设计原则

### 1. 原则一：绝对禁止在 Docker 内通过 QEMU 软件模拟编译
- **反模式**：在 x86_64 构建机上使用 `docker buildx` + QEMU 模拟 aarch64 (ARM64) 执行源码编译（如 `cargo build --release`、`go build`、`cmake/make`）。QEMU 在用户态模拟指令集的 CPU 开销极大，编译复杂依赖时耗时膨胀 15~20 倍（常需 1.5~3 小时），且极易触发内存耗尽或超时挂起。
- **标准方案**：**“交叉编译二进制在前，纯拷贝装配在后”**。在构建机裸机环境利用专用工具链（Rust `cross` / Go `GOARCH=arm64`）在 2 分钟内编译好原生静态二进制；Docker 打包阶段仅执行文件拷贝（COPY），多架构打包与推送控制在 30 秒内。

### 2. 原则二：CI 运行环境与本地网络镜像严格解耦
- **反模式**：在 `Dockerfile` 或源码配置文件中硬编码国内镜像源（如 `rsproxy.cn`、国内 npm 镜像源）。
- **标准方案**：GitHub Actions 运行节点位于微软 Azure 海外机房（美西/西欧），反向连接国内镜像源易遭遇高丢包率、TLS 握手超时或网络挂起。流水线必须在前置阶段剥离或替换本地私有镜像源，直接走官方源拉取依赖。

### 3. 原则三：严格流水线单向依赖与制品解耦
- 采用规范阶段依赖：`build-frontend` -> `build-binaries` -> [`publish-release`, `docker`]。
- Docker 打包任务强依赖 `build-binaries` 的编译产物，杜绝同一套代码在不同 Job 中重复编译。

---

## 二、标准容器文件规范 (双模模式)

项目中维护两个 Dockerfile，严格分离“自动化发布打包”与“本地单机开发构建”：

### 1. 发布专用多架构模板 (`Dockerfile.release`)
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

### 2. 本地开发兼容构建模板 (`Dockerfile`)
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

## 三、标准化 GitHub Actions 工作流模板 (`.github/workflows/release.yml`)

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

  # 阶段 3: 发布 GitHub Release 静态包与校验和
  publish-release:
    name: Publish GitHub Release Assets
    needs: build-binaries
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Download all package artifacts
        uses: actions/download-artifact@v4
        with:
          path: downloaded-artifacts
          pattern: package-*
          merge-multiple: true

      - name: Generate Checksums
        run: |
          mkdir -p release-assets
          cp downloaded-artifacts/* release-assets/
          cd release-assets
          sha256sum * > checksums.txt

      - name: Upload assets to GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: release-assets/*
          tag_name: ${{ inputs.tag || github.ref_name }}
          generate_release_notes: true
          draft: false
          prerelease: false

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
```

---

## 四、新项目接入检查清单 (Checklist)

1. **替换占位符**：
   - `<项目名>`：如 `nexdns`、`blockyx`
   - `<二进制名称>`：编译生成的 ELF / 可执行文件名称
   - `<镜像名称>`：GHCR 发布的镜像路径（全小写）
   - `<服务端口>`：服务监听的主机映射端口
2. **忽略中间目录**：
   - 在 `.gitignore` 中追加 `docker-bin/` 与 `release-packages/`。
3. **保留并发取消配置**：
   - 务必配置 `concurrency: cancel-in-progress: true`，杜绝重复推送 Tag 时前序任务持续耗尽免费构建额度。
4. **统一采用 musl 纯静态链接**：
   - Linux 构建必须指定 `*-unknown-linux-musl`，避免目标 Alpine 镜像由于缺乏 glibc 产生 `file not found` 无法执行的问题。

---

## 五、常见故障与排错指南

| 现象 | 根因分析 | 解决措施 |
|---|---|---|
| **Docker 构建持续数小时不结束** | 使用 QEMU 在 x86 上模拟编译 ARM64 源码 | 采用本规范的 `Dockerfile.release`，直接解压复制宿主机编译好的静态二进制，耗时降至 30 秒内。 |
| **Cargo / npm 拉取依赖握手超时** | 配置文件写死了国内镜像源（如 rsproxy/npmmirror） | 在构建脚本开头执行 `rm -f .cargo/config.toml`，确保 CI 环境直连官方源。 |
| **容器启动报 `exec: not found`** | 编译时使用了默认 glibc (gnu)，而运行基础镜像为 Alpine (musl) | Linux 编译目标统一切换至 `x86_64-unknown-linux-musl` 与 `aarch64-unknown-linux-musl`。 |
| **反复修改 Tag 触发多个 Run 浪费配额** | 工作流缺少并发控制组 | 在根级别声明 `concurrency: { group: ..., cancel-in-progress: true }`。 |
