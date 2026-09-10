# macOS 原生平台骨架

本目录承载 macOS 原生平台的本地开发骨架。第一批范围只覆盖 SwiftPM 工程、本地构建入口、FFI smoke 测试入口与 host scaffold，不包含可安装的 InputMethodKit bundle。

## 前置条件

```bash
xcode-select -p
swift --version
cargo --version
```

`xcode-select -p` 指向 `/Library/Developer/CommandLineTools` 时，SwiftPM 构建可继续使用；后续接入 Xcode bundle、签名与系统注册前，需要将 active developer directory 切到完整 Xcode。

## 本地构建

在仓库根目录执行：

```bash
./build.sh macos
```

脚本会先编译 Rust FFI 静态库，再构建 `GannyuInputMethodHost` 与 `GannyuMacOSSmoke`。

## smoke 测试

```bash
bash share/platforms/macos/smoke.sh
```

默认从 `share/resources/manifest.toml` 读取资源，执行一次 `retrieve gau` 与 `compose 吹牛`。本机覆写写入 `share/platforms/macos/test_local.env`，该文件不进入 Git。

## host scaffold

```bash
bash share/platforms/macos/run_host.sh
```

该入口加载 FFI pipeline 并启动 `IMKServer`。当前阶段只验证原生宿主、资源路径与 FFI 链路，不向系统注册输入法。
