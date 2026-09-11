# iOS 平台

该目录提供 iOS 宿主 App 与 Keyboard Extension。宿主 App 负责地区选择；键盘扩展从同一 App Group 读取选择，并通过加密嵌入的 Rust FFI 加载对应地区资源。地区列表始终由 `resources/manifest.toml` 派生，新增地区无需改 iOS 界面代码。

正式构建前，复制 `Config/Signing.xcconfig.example` 为 `Config/Signing.xcconfig`，填入 Apple Team ID、App Group 与正式 Bundle ID。然后从仓库根目录运行：

```bash
GANNYU_RESOURCE_KEY_FILE=/path/to/resource-key bash share/platforms/ios/build.sh
```

脚本会为真机和模拟器构建加密资源 FFI 的 XCFramework，再 archive 宿主 App 和键盘扩展。它不会安装、上传或发布。
