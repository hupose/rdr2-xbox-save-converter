# 使用 xbcsmgrrev 下载 RDR2 Xbox 云存档

这里使用的是 **[LukeDevsE](https://github.com/LukeDevsE) / [xbcsmgrrev](https://github.com/LukeDevsE/xbcsmgrrev)**。它是第三方开源工具，不是 Microsoft、Xbox 或 Rockstar 官方软件。

## 风险边界

`xbcsmgrrev 4.0.1` 使用微软设备码流程，正常情况下你只需要在微软官方网页输入密码，不应把密码输入程序。该版本会把 refresh token 明文保存到本机，因此推荐使用临时 Windows 用户或虚拟机，并在完成后清理令牌、撤销授权。

Release EXE 没有数字签名，也无法仅凭源码证明二进制逐字对应。请只从官方 GitHub Release 下载，核对文件哈希，使用 Microsoft Defender 扫描，并以普通用户运行。

## Xbox 端准备

1. 确认 Xbox 使用的账号就是目标存档所属账号。
2. 联网启动 RDR2，进入故事模式并新建一次手动存档。
3. 正常退出到 Xbox 主界面，保持联网一段时间以完成同步。
4. 不要删除主机上的存档。

## Windows 下载步骤

1. 从 [xbcsmgrrev Releases](https://github.com/LukeDevsE/xbcsmgrrev/releases) 下载最新可信版本。本文验证过的版本为 `4.0.1`。
2. 用 Defender 扫描解压目录，不要关闭 SmartScreen或添加排除项。
3. 以普通用户运行 `XboxCsMgr.Client.exe`。
4. 程序显示设备码和 `https://www.microsoft.com/link` 后，自己在浏览器中打开该微软地址。
5. 密码只能输入微软官方 HTTPS 页面。不要把设备码、登录跳转地址、密码或 token 发给任何人。
6. 授权页可能显示 `Prism Launcher`，因为程序借用了它的 OAuth 客户端 ID。浏览器显示完成后再返回程序。
7. 在左侧找到 `Red Dead Redemption 2`，双击后展开右侧保存容器。
8. 每个顶层容器分别下载到独立空目录；不要把多个容器混在同一目录，以免同名 atom 被覆盖。
9. 下载整个容器，保留所有 `Save Game Data`、`Save Game Header` 和其他 atom，不改名、不编辑。
10. 将含有 `SRDR...` 文件夹的目录打成 ZIP，并保持原始相对路径。转换器会自动定位每个 SRDR 文件夹内的一对 Data/Header。

## 完成后的清理

1. 完全退出 `XboxCsMgr.Client.exe`。
2. 删除：

   ```text
   %LOCALAPPDATA%\xbcsmgrrev\reftoken
   ```

3. 打开微软账号的[应用和服务权限页面](https://account.live.com/consent/Manage)，撤销本次临时使用的 `Prism Launcher` 授权。
4. 若使用了临时 Windows 用户或虚拟机，删除用户或恢复快照。

如果你本来就在使用真正的 Prism Launcher，撤销权限会使它下次需要重新登录。

## 常见问题

### 看不到 RDR2

确认账号正确、Xbox 刚联网运行并保存过一次。等待同步后重启下载器，不要从论坛复制来源不明的 SCID/PFN。

### 没有存档或下载失败

这可能是同步尚未完成、授权失败，或未公开接口已变化。退出并清理 token 后再重试一次；不要尝试写入、删除或替换云端内容。

### SmartScreen 阻止程序

不要全局关闭 SmartScreen。重新确认下载来源、哈希和 Defender 扫描；不能接受未知发布者风险时，应从固定源码自行构建。

