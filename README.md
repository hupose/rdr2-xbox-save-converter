# RDR2 Xbox Save Converter

一个本地、离线、开源的 GUI 工具，把 **Red Dead Redemption 2 / 荒野大镖客：救赎 2** 的 Xbox Connected Storage 剧情存档转换为 Windows PC 存档。

- macOS 与 Windows 原生 GUI；
- 不登录 Xbox、不联网、不上传存档；
- 不修改输入 ZIP；
- 转换前后严格验证 RSAV 结构、10 个 CHKS 和 AES 往返；
- Release 和源码都**不包含 Rockstar 游戏密钥**。

> 本项目只处理剧情模式存档，不支持 Red Dead Online。

## 下载

在 [Releases](https://github.com/hupose/rdr2-xbox-save-converter/releases) 下载对应系统压缩包：

- Windows x64：`rdr2-xbox-save-converter-windows-x64.zip`
- macOS Apple Silicon：`rdr2-xbox-save-converter-macos-arm64.zip`
- macOS Intel：`rdr2-xbox-save-converter-macos-x64.zip`

构建产物没有商业代码签名。Windows SmartScreen 或 macOS Gatekeeper 可能提示未知开发者；请先核对 Release 中的 `SHA256SUMS.txt`。不接受这一风险时，请按下文从源码构建。

## 第一步：下载 Xbox 云存档

本工具不负责登录 Xbox。请使用 [LukeDevsE/xbcsmgrrev](https://github.com/LukeDevsE/xbcsmgrrev) 在 Windows 上把 RDR2 的全部保存容器下载下来。

建议：

1. 只从项目官方 Release 下载；
2. 密码只输入微软官方设备码登录网页，不输入第三方程序；
3. 每个顶层容器下载到独立空目录；
4. 保留全部 `Save Game Data` 与 `Save Game Header`；
5. 下载完成后退出程序，删除其本地 refresh token，并从微软账号权限页撤销临时授权；
6. 把包含各 `SRDR...` 文件夹的结果打成 ZIP，文件夹结构保持不变。

`xbcsmgrrev` 是第三方工具；使用前请自行审查源码和风险。更详细步骤见 [docs/xbcsmgrrev-guide.zh-CN.md](docs/xbcsmgrrev-guide.zh-CN.md)。

## 第二步：准备 key 配置

为了避免仓库和 Release 直接分发游戏密钥，本程序需要两把 AES-256 key，均由用户从公开资料自行取得：

- Xbox：打开 [2013 年 Xbox 360 Save Block Editor 帖子](https://community.wemod.com/t/gta-v-save-block-editor-0-0-3-x360-source/2899)，在页面中查找 `GTAV=`；
- PC：打开 [HTOS 的 rstar_crypt.py](https://github.com/hzhreal/HTOS/blob/31fca60508251e591afe261b927d73c76a7b3503/data/crypto/rstar_crypt.py)，查找 `PC_KEY`。

不要把 key 发到 issue、截图、聊天或 fork 中。

启动程序后点击“创建空白配置模板”，按界面显示的位置编辑：

```toml
xbox_save_key = "填入 64 个十六进制字符"
pc_save_key = "填入 64 个十六进制字符"
```

默认配置位置：

- Windows：`%APPDATA%\hupose\RDR2 Xbox Save Converter\config\keys.toml`
- macOS：`~/Library/Application Support/hupose.RDR2 Xbox Save Converter/keys.toml`

也可以把名为 `rdr2-converter.toml` 的配置放在可执行文件旁边，作为便携模式配置。填好 key 的配置已被 `.gitignore` 排除，仍请避免放入云同步目录。

## 第三步：转换

1. 选择 `xbcsmgrrev` 结果 ZIP；
2. 点击“检查存档”；
3. 选择一个尚不存在的输出目录；
4. 点击“转换并严格验证”。

成功后会生成 `SRDR30000` 等 PC 存档和 `conversion_manifest.json`。程序不会覆盖已有输出目录。

把目标文件复制到：

```text
%USERPROFILE%\Documents\Rockstar Games\Red Dead Redemption 2\Profiles\<Profile ID>
```

导入前备份整个 `Profiles`，暂时关闭 Rockstar Games Launcher 云存档。优先测试最新手动档；成功进入后立即另存一个新的 PC 手动槽位，再恢复云同步。

## 校验内容

转换仅在以下条件全部满足后才写出最终目录：

- Xbox atom 头版本、声明长度、标题与时间戳合法；
- Xbox key 能解出 `RSAV` v3 与 `ENDS`；
- 固定的 9 个块 ID、动态块尺寸、偏移、`CODE/S000…S008` 标签自洽；
- 恰好 10 个 CHKS，覆盖范围及 seeded JOOAT 全部正确；
- Xbox 原密文可以逐字节回加密；
- PC 头、头校验和、PC AES 包装可完整解密与回加密；
- Xbox 与 PC 解密后的 RSAV 主体逐字节相同。

较早的 RDR2 存档存在多代合法块尺寸，因此程序验证文件自己的动态布局，而不是只接受一个现代 PC 尺寸表。

## 从源码构建

安装 Rust 1.85 或更高版本：

```bash
cargo test --locked
cargo build --release --locked
```

可选的无界面命令：

```bash
rdr2-xbox-save-converter --convert INPUT.zip OUTPUT_DIR KEYS.toml
```

## 安全与隐私

- 程序没有 HTTP/网络依赖，也没有遥测；
- key 只在本地读取，不写入日志或 manifest；
- 不包含用户存档、测试存档或账号数据；
- 输入文件只读，输出采用临时目录完整验证后再原子重命名；
- 请勿把真实存档或 key 提交到公开 issue。

安全问题请参阅 [SECURITY.md](SECURITY.md)。

## 免责声明

本项目与 Rockstar Games、Take-Two Interactive、Microsoft 或 Xbox 无关。请仅处理你有权访问的本地剧情存档。所有商标归其各自所有者所有。

