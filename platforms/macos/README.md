# macOS 原生平台骨架

本目录承载 macOS 原生平台的本地开发骨架。当前阶段覆盖 SwiftPM 工程、本地构建入口、FFI smoke 测试入口、InputMethodKit app bundle 打包路径与本地安装脚本，不包含完整输入循环与系统级候选窗行为。

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

脚本会先编译 Rust FFI 静态库，再构建 `GannyuInputMethodHost` 与 `GannyuMacOSSmoke`，随后组装 `share/build/macos/GonnyuInputMethod.app`。

## smoke 测试

```bash
bash share/platforms/macos/smoke.sh
```

默认从 `share/resources/manifest.toml` 读取资源，执行一次 `retrieve gau` 与 `compose 吹牛`。本机覆写写入 `share/platforms/macos/test_local.env`，该文件不进入 Git。

## bundle 自检

```bash
bash share/platforms/macos/bundle_smoke.sh
```

该入口校验 `Info.plist`、bundle 结构、controller class 配置与 app 内可执行文件的自启动链路。

## host scaffold

```bash
bash share/platforms/macos/run_host.sh
```

该入口运行打包后的 app 内可执行文件，加载 FFI pipeline 并启动 `IMKServer`。

## 本地安装

```bash
bash share/platforms/macos/install_local.sh
```

脚本优先使用钥匙串内的 Apple Development 身份签名（也可用 `GANNYU_MACOS_SIGN_IDENTITY` 指定），复制到 `~/Library/Input Methods/`，再调用 macOS 的 Text Input Source Services 完成登记。完成后重新打开“系统设置 → 键盘 → 输入法”即可添加；不需要注销或重启。
