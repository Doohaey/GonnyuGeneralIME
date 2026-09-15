# iOS 平台

该目录提供 iOS 宿主 App 与 Keyboard Extension。宿主 App 负责地区选择；键盘扩展从同一 App Group 读取选择，并通过加密嵌入的 Rust FFI 加载对应地区资源。地区列表始终由 `resources/manifest.toml` 派生，新增地区无需改 iOS 界面代码。

正式构建前，复制 `Config/Signing.xcconfig.example` 为 `Config/Signing.xcconfig`，填入 Apple Team ID、App Group 与正式 Bundle ID。然后从仓库根目录运行：

```bash
GANNYU_RESOURCE_KEY_FILE=/path/to/resource-key bash share/platforms/ios/build.sh
```

脚本会为真机和模拟器构建加密资源 FFI 的 XCFramework，再 archive 宿主 App 和键盘扩展。它不会安装、上传或发布。

本地真机安装可先完成未签名编译，再使用 Xcode 已安装的开发 provisioning profile 自动嵌入、签名并安装：

```bash
GANNYU_IOS_DEVICE_ID="设备 UDID" \
  bash share/platforms/ios/install_device.sh
```

脚本按 App Bundle ID 自动匹配主 App 与键盘扩展的 profile，不读取或上传私钥；profile 和证书均来自本机 Xcode/钥匙串。
