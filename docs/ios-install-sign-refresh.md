# iPhone / iPad 安装、签名与续签教程

适用于本 fork 的 HGhub 1.2.1。核对日期：2026-11-12。

## 先看结论

- **没有付费 Apple 开发者账号也可以尝试安装**：使用自己的普通 Apple 账户，通过电脑上的 Sideloadly 或 AltStore Classic 进行个人签名。
- **不能像 Android APK 一样，在 iPhone 上点一下未签名 IPA 就安装。** 首次安装需要 Windows 电脑或 Mac、数据线，以及你自己操作 Apple 账户登录和双重认证。
- 免费个人签名通常 **7 天到期**，到期前需重新签名（“刷新/续签”）。这不是永久安装，也不是 App Store / TestFlight 分发。
- 不需要自己部署这个客户端项目。但登录、注册、内容和播放依赖外部服务；安装成功不代表这些服务会一直可用。上游 README 已说明其维护范围。
- 本 IPA 已做构建与包结构检查，**尚未完成真实 iPhone 的安装、登录与播放测试**。下面是官方工具支持的安装方式，不是对所有系统版本的兼容保证。

## 1. 下载正确的文件

当前版本：**HGhub_1.2.1_ios-arm64-unsigned.ipa**

- [直接下载 iOS IPA](https://github.com/nbbk/HGhub/releases/download/1.2.1/HGhub_1.2.1_ios-arm64-unsigned.ipa)
- [查看 1.2.1 发布页](https://github.com/nbbk/HGhub/releases/tag/1.2.1)

这是 iOS 15 或更新系统的 arm64 真机包，不是模拟器包；`unsigned` 表示需要你个人签名。不要下载 APK、macOS DMG、Windows EXE 或 GitHub 的 Source code 压缩包来安装到 iPhone。

如需核验下载完整性，IPA 的 SHA-256 为：

```text
4fbe524d34a9e1b37963cea1624473d623b0491de93184dd125f3629736768ee
```

Windows PowerShell：
```powershell
Get-FileHash .\HGhub_1.2.1_ios-arm64-unsigned.ipa -Algorithm SHA256
```

Mac 终端：
```sh
shasum -a 256 HGhub_1.2.1_ios-arm64-unsigned.ipa
```

## 2. 推荐方法：Sideloadly（Windows / Mac）

对于只安装 HGhub 的用户，优先用这个方法。不要同时用两套工具反复覆盖同一个 App。

### 2.1 准备电脑和手机

1. 从 [Sideloadly 官网](https://sideloadly.io/) 下载适合电脑的版本，不使用搜索广告、网盘或“免签版”。
2. **Windows：** 按 [官方 FAQ](https://sideloadly.io/faq) 安装其要求的 Apple iTunes、iCloud 组件。官方常规路径使用 Apple 网站版，不是 Microsoft Store 版；下载入口以官网当前说明为准。已有冲突版本时先按官方说明处理，并重启电脑。
3. **Mac：** 安装官网提供的 Sideloadly；较新 macOS 使用 Finder 管理 iPhone，一般不需要另装 iTunes。
4. 用可传输数据的数据线连接 iPhone，解锁手机。在手机上点“信任此电脑”，输入手机锁屏密码。确认 Finder / iTunes 能看到设备。
5. 电脑与手机保持联网，准备自己的 Apple 账户。不要把密码、验证码、恢复密钥发给仓库维护者、聊天助手或陌生“代签”人员。

### 2.2 签名并安装 IPA

1. 打开 Sideloadly，在设备选择框中选中你的 iPhone。
2. 将刚下载的 `.ipa` 拖入 Sideloadly 的 IPA 区域。
3. 在 Apple ID 栏填写自己的 Apple 账户。使用一个你能长期保留并完成双重认证的账户。
4. 首次操作保留默认选项，不必改 Bundle ID，也不要启用注入、破解或其他高级修改。
5. 如希望电脑帮助自动续签，可启用 **自动刷新 / Automatic Refresh** 选项。不同版本的按钮位置可能不同，以官方界面为准。
6. 点 **Start**。需要时，仅在你从官网下载的工具登录窗口中输入密码，并自行完成 Apple 双重认证。
7. 等待工具显示安装完成，再检查手机桌面是否出现 HGhub。

第三方签名工具需要处理你的 Apple 登录凭据。虽然工具官方说明了其登录方式，仍应自行评估信任风险；本项目不保证第三方工具绝对安全。不需要向本项目提交证书、私钥或 Apple 密码。免费账户流程不要想当然地用“App 专用密码”替代正常登录，按工具当前提示操作。

### 2.3 手机上信任签名，开启开发者模式

如果首次启动被拦截：

1. 打开 **设置 → 通用 → VPN 与设备管理**（旧系统可能叫“描述文件与设备管理”）。
2. 在开发者 App 中找到本次签名使用的 Apple 账户，点“信任”，按系统要求确认。新系统如要求重启完成信任，照提示操作。
3. **iOS 16 及更新版本：** 打开 **设置 → 隐私与安全性 → 开发者模式**，开启后重启手机，解锁并确认开启。iOS 15 没有这一开关。
4. 再打开 HGhub。

没有看到开发者模式时，先完成一次有效的开发签名安装、连接可信电脑，再检查系统提示与 [Apple 官方说明](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device)。不要为了安装这个 App 关闭系统安全保护、安装陌生根证书或陌生设备管理配置。

## 3. 最重要：每 7 天刷新签名

免费个人签名通常只有 7 天有效期；以工具显示的到期时间为准。**建议每 5～6 天主动刷新一次，不要等到最后一刻。** 只打开 HGhub 不会给签名续期。

### 3.1 最稳妥的手动刷新

1. 保留电脑上的 IPA 和 Sideloadly。
2. 连接、解锁并信任同一台 iPhone。
3. 在 Sideloadly 选择 HGhub 的 IPA，使用**相同 Apple 账户和相同 Bundle ID**。
4. 再次点 Start，等待重新签名并覆盖安装成功。
5. 在手机上打开 App 验证。

通常覆盖安装可保留同一 App 的本地数据，但不要把它当作备份保证。**不要为了续签先删除 HGhub**，删除会丢失其本地数据；换账户、换 Bundle ID 也可能形成另一份 App，而不是更新原来的。

### 3.2 自动刷新如何工作

Sideloadly 的自动刷新由电脑上的 **Sideloadly Daemon** 帮助执行。它不是手机离开电脑后仍能无条件续签的服务。

必须满足：
- 电脑开机、联网，后台组件正在运行；
- iPhone 能被电脑发现，通过 USB 或已配置好的同一局域网 Wi-Fi；
- 自动刷新选项已经为该 App 启用；
- Apple 登录及签名服务没有要求你重新认证。

启用 Wi-Fi 连接：
- Windows / 旧 Mac 的 iTunes：打开设备摘要页，勾选“通过 Wi-Fi 与此 iPhone 同步”，点击应用或同步。
- 新 Mac 的 Finder：侧边栏选择 iPhone，在“通用”中勾选“连接 Wi-Fi 时显示此 iPhone”，点击应用。
- 初次设置用数据线完成。以后让电脑和手机连接同一网络；设备找不到时，解锁手机或直接插线。

电脑关机、休眠、离线或网络隔离，都可能导致自动刷新失败。出门前手动刷新一次更可靠。

### 3.3 已经过期打不开怎么办

先不要卸载。将手机连接电脑，使用原来的账户与 Bundle ID，重新签名覆盖安装。恢复后检查数据是否还在。若工具提示需要重新登录，自己在官方工具中完成认证即可。

**续签不等于升级：** 想更新 HGhub，需要另下载新版本 IPA，再用同一账户、相同标识覆盖安装；只刷新旧 IPA 不会自动变成新版本。

## 4. 备选方法：AltStore Classic

这里指 **AltStore Classic + AltServer**，不是 AltStore PAL。PAL 不是这份任意未签名 IPA 的通用导入方案。

### 4.1 在电脑安装 AltServer

- [Windows 官方教程](https://faq.altstore.io/altstore-classic/how-to-install-altstore-windows)：按要求安装 Apple 网站版 iTunes 和 iCloud，再安装 AltServer。
- [Mac 官方教程](https://faq.altstore.io/altstore-classic/how-to-install-altstore-macos)：安装当前兼容版本的 AltServer，移入“应用程序”并运行。若特定版本提示额外步骤，只按对应官方教程处理。

连接手机、完成“信任此电脑”，并启用 Finder / iTunes 的 Wi-Fi 同步。电脑右下角托盘或 Mac 菜单栏打开 AltServer，选择 **Install AltStore → 你的 iPhone**，使用自己的 Apple 账户完成登录和双重认证。

在 iPhone 上信任开发者签名；iOS 16+ 开启开发者模式，步骤同上。

### 4.2 将 HGhub 导入 AltStore

1. 用 Safari 下载本教程中的 IPA，在“文件”App 中确认下载完成。
2. 打开 AltStore，进入 **My Apps**，点 **+** 并选择 IPA；也可尝试通过“文件”的分享菜单用 AltStore 打开。
3. 按提示登录自己用于 AltStore 的 Apple 账户。
4. 保持 AltServer 正在电脑上运行，并确保电脑能连接到手机。等待 HGhub 安装完成。

免费账户通常最多同时安装 3 个此类签名 App，**AltStore 本身也占一个名额**。签名还受 Apple 的 App ID 配额等限制；遇到额度提示，按工具显示的限制和到期时间处理，不使用绕过限制的方案。

### 4.3 用 AltStore 刷新

1. 在签名到期前，让电脑运行 AltServer，手机与其 USB 连接或在同一可互通的 Wi-Fi 网络。
2. 打开 AltStore → **My Apps → Refresh All**。
3. 等待 HGhub 和 AltStore 自身都刷新成功，检查各自剩余天数。
4. 可开启手机的后台 App 刷新，让 AltStore尝试自动续签，但仍需电脑可达；不要只依赖后台刷新。

如果 **AltStore 自己已经过期**，先用电脑 AltServer 重新安装 AltStore，再刷新 HGhub。尽量覆盖安装，不先删除 App。

## 5. 常见问题

| 现象 | 优先检查 |
| --- | --- |
| 手机点 IPA 只显示文件，不能安装 | 这是未签名包，必须先用上述工具签名并安装。 |
| 工具找不到手机 | 解锁、信任电脑、换数据线/USB 接口；先确认 Finder/iTunes 能识别，再按官方说明检查 Windows Apple 组件。 |
| “不受信任的开发者” | 设置 → 通用 → VPN 与设备管理，信任自己签名的账户。 |
| 要求开发者模式 | iOS 16+ 在隐私与安全性中开启，重启后再次确认。 |
| 数天后突然打不开 | 先查签名是否到期，用原账户和同一 Bundle ID 覆盖续签，不先卸载。 |
| 自动刷新没有成功 | 电脑是否开机、后台工具是否运行、同网是否可互通、Apple 是否要求重新认证；插线手动刷新。 |
| 提示 App 数量/App ID 上限 | 免费账户有配额。只移除自己确定不再需要的 App，注意备份；App ID 配额恢复可能需要等待工具显示的期限。 |
| 签名成功但仍闪退/安装失败 | 检查 iOS 版本、IPA 下载完整性和工具日志；可能是包兼容性、权限或运行时问题，不能都归因于签名。反馈时隐去账号、设备标识和令牌。 |
| 可以打开，但登录/注册/播放不可用 | 可能是外部服务或接口变化，不等于签名失败；本教程不提供绕过服务限制的方法。 |

## 6. 安全与维护提醒

- 不把 Apple 密码、双重认证验证码、恢复密钥上传到 GitHub、Issue、聊天或截图。
- 不购买陌生“永久免签”，也不信任来路不明的企业证书、MDM 或根证书。免费个人签名不是永久分发渠道。
- 签名只解决 iOS 允许安装和启动的问题，不证明 App、外部服务或第三方工具没有漏洞。
- 项目客户端签名用的 Apple 账户，与 HGhub 内部的服务账户是两回事。
- 本教程只新增文档，不改变原应用登录、注册、网络访问或其他源代码行为。

## 官方参考

工具界面、系统要求与 Apple 限制可能变化，遇到差异优先查看：
- [Sideloadly 官网](https://sideloadly.io/) / [官方 FAQ](https://sideloadly.io/faq)
- [AltStore Classic：Windows 安装](https://faq.altstore.io/altstore-classic/how-to-install-altstore-windows)
- [AltStore Classic：macOS 安装](https://faq.altstore.io/altstore-classic/how-to-install-altstore-macos)
- [AltStore：My Apps 与刷新](https://faq.altstore.io/altstore-classic/your-altstore)
- [Apple：开启开发者模式](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device)
