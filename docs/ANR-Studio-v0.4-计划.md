# ANR Studio v0.4 计划：分发与诊断

> 起草：2026-11-21 ｜ 基线：`86928d1`（v0.3.0 已发布）
> 目标：把「能装、能用」变成「敢让普通用户装」——签名、自动更新、崩溃可诊断、日志能看数据。
> 定位不变：不做注入、不做内存读写、不重写安装逻辑。

---

## 0. 一页摘要

| # | 条目 | 优先级 | 预估 | 依赖 |
|---|---|---|---|---|
| 1 | 代码签名（Authenticode） | P0 | 3–5 人日（含踩证书流程） | 先拿到证书/签名服务 |
| 2 | GitHub Actions 发布流水线 | P0 | 2 人日 | 签名方案定了才有意义 |
| 3 | 自动更新（`tauri-plugin-updater`） | P1 | 4 人日 | 依赖 1 + 2（更新包要验签） |
| 4 | 崩溃捕获与下次启动提示 | P0 | 2 人日 | 无 |
| 5 | 日志实时跟随 | P1 | 2 人日 | 无 |
| 6 | 帧数 / 延迟统计面板 | P1 | 4 人日 | **需要一份真实 `nr_fg-*.log` 样本** |
| 7 | 诊断包升级（GPU/驱动/脱敏） | P1 | 3 人日 | 无 |
| 8 | 代理 DLL 手动覆盖 | P1 | 1.5 人日 | 无 |
| 9 | CSP 收紧 + 外链加固 | P1 | 1 人日 | 无 |
| 10 | 单实例 | P2 | 0.5 人日 | 无 |

合计约 **23–25 人日**（单人、含联调）。建议顺序：**4 → 9 → 8 → 7 → 1 → 2 → 3 → 5 → 6 → 10**，即先把不依赖外部条件的安全项做完，再进证书/更新这条长链路。

发布门槛（v0.4 的定义）：`v0.3.0 → v0.4.0` 能在应用内点「更新」完成升级且验签通过；安装包带有效 Authenticode 签名；断网、双击无响应、驱动异常三种情况下都有可提交的诊断材料。

---

## 1. 现状与证据（v0.3.0 实测）

| 项 | 现状 | 证据 |
|---|---|---|
| 代码签名 | 无。`tauri.conf.json` 只配了 digest / timestamp / NSIS 安装模式 | `app/src-tauri/tauri.conf.json:44` `timestampUrl`；无 `certificateThumbprint` / `signCommand` |
| CI | 无 `.github` 目录，发布为手工 `tauri build` + 手工上传 | 仓库根目录；本次 0.3.0 即我手工构建后上传 |
| 自动更新 | 无任何 `tauri-plugin-*`，用户只能手动下载覆盖 | `app/src-tauri/Cargo.toml:17` 只有 `tauri` 本体 |
| 崩溃 | release `panic="abort"`，无 panic hook、无崩溃文件 | `Cargo.toml:32` |
| 日志 | 一次读全文再取尾部 400 行，要手动点刷新 | `logs.rs:8-15`（`fs::read` 整个文件）；`app/src/views/Logs.tsx:82` 手动按钮 |
| 帧数 | 只展示日志原文，不解析任何数值 | 无解析代码 |
| 诊断包 | `system.txt` 只有 OS / 架构 / 目录 / 状态，没有显卡与驱动；版本号已修但无脱敏 | `logs.rs:52-58`；`lib.rs:1185` `make_diag` |
| 代理 DLL | 只能自动判定（有 `winmm.dll` 无 `dxgi.dll` 时强制 dxgi），用户无法选 | `lib.rs` 的 `proxy_hint` |
| CSP | `null` | `tauri.conf.json:28` |
| 外链 | `cmd /C start "" <url>` 拼串 | `lib.rs:1543` `open_url` |
| 单实例 | 无插件，双开会并发写同一游戏目录 / 同一 `settings.json` | `Cargo.toml` 全量依赖 |

---

## 2. 逐项设计

### P0-4 崩溃捕获与下次启动提示

**问题**：release 用 `panic = "abort"`，闪退后没有任何信息，用户只能卸载走人；我们也无法复现。

**方案**

1. `src-tauri/src/crash.rs`：`std::panic::set_hook`，把 `panic message + location + backtrace + 版本 + 当前视图 + 最近任务` 写到 `<数据目录>/crash/<时间戳>.txt`。`panic="abort"` 下 hook 仍会在 abort 前执行，所以能落盘。
2. 启动时（`run()` 的 setup）扫描 `crash/` 下未上报的文件：有则 emit `crash://pending`，前端弹一条常驻 toast +「生成诊断包并复制到剪贴板」按钮；上报/忽略后写 `.ack` 标记。
3. 前端错误边界：`App.tsx` 外包一层 `ErrorBoundary`，React 渲染异常也落同一目录（`window.onerror` / `unhandledrejection` 一并挂上）。

**涉及**：新增 `app/src-tauri/src/crash.rs`；`lib.rs` 的 `run()`；新增 `app/src/components/ErrorBoundary.tsx`；`App.tsx`；`store.ts` 一条 `crashPending` 状态。

**验收**：手动 `panic!()` 后进程退出，`crash/` 出现完整文件；重启应用能看到提示并一键出诊断包；前端渲染炸掉不会白屏。

**风险**：`panic=abort` 时 hook 里做太多工作可能二次崩溃 → hook 内只做文件写入，不分配大对象、不调用 Tauri API。

---

### P1-9 CSP 收紧与外链加固

**问题**：`csp: null` 意味着没有任何内容策略；`open_url` 把 URL 拼进 `cmd /C start`，URL 里带 `&` 会被 cmd 当命令分隔符。

**方案**

1. `open_url` 改用 `ShellExecuteW`（`windows` crate 已在依赖树里，或直接 `unsafe extern "system"`），只接受 `https://` 与 `http://`。
2. CSP 取值需要先解决一个矛盾：前端用 `vite-plugin-singlefile` 把 JS **内联**进 `index.html`，`script-src 'self'` 会直接白屏。两条路：
   - **A（推荐，代价小）**：保留内联，配 `script-src 'self' 'unsafe-inline'`，但把其余全部关死：
     `default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`
     —— 依然能挡掉远程脚本、外连、iframe 注入，收益真实。
   - **B（更严，代价大）**：放弃单文件打包，改成 hash 白名单 CSP，需要改 `vite.config.ts` 并接受多文件产物（安装包体积变化不大，但「单文件」这个特性没了）。
3. 前端所有外链统一走 `open_url`，禁止 `window.open`。

**验收**：`npm run build` 后应用能正常渲染（无 CSP 报错）；`img-src` 仍能显示 base64 封面；`open_url` 传 `https://a.com/?x=1&y=2` 能正常打开；传 `file:///C:/Windows/System32/calc.exe` 被拒。

**风险**：CSP 设错会白屏 → 先在调试构建里验证一版，再定稿。

---

### P1-8 代理 DLL 手动覆盖

**问题**：现在只有一条自动规则（`proxy_hint`）。遇到「游戏已有 winmm 补丁」「别的注入器占了 dxgi」「反作弊只放行某个名字」就没有出路。

**方案**

1. 后端新增 `set_proxy(id, dll)`：只允许 `dxgi.dll` / `winmm.dll` / `version.dll`；执行顺序：确认未运行 → 快照 → 用目标名重跑官方 `install.ps1`（脚本本身支持第三个参数指定代理）→ 后置校验 `nr_fg_install.json.proxy == dll`。
2. 抽屉「安装信息」里把「代理 DLL」从只读改成三段选择 + 一句说明（`dxgi` 通用、`winmm` 适配 Vulkan/有 dxgi 冲突、`version` 兜底）。
3. 失败时走现有失败闭环（重试 / 诊断包 / 回滚）。

**涉及**：`lib.rs` 新命令；`store.ts` 新 action；`Drawer.tsx` 安装信息区。

**验收**：切换后 `nr_fg_install.json`、实际 DLL 名、抽屉显示三者一致；切回自动（重装）能恢复。

**风险**：改代理名是一次完整的重装，包没缓存时要下载 → 必须先确认游戏未运行（已有 `game_running` 实时判定可复用）。

---

### P1-7 诊断包升级（GPU / 驱动 / 脱敏）

**问题**：`system.txt` 只有 OS 与架构，用户报「画面撕裂」时我们连显卡型号都看不到；同时日志与 ini 里有完整用户名路径。

**方案**

1. 采集（在 blocking 线程里跑，允许慢 1–2 秒）：
   - GPU：`Get-CimInstance Win32_VideoController`（Name / DriverVersion / DriverDate / AdapterRAM）
   - CPU / 内存：`Win32_Processor.Name`、`Win32_ComputerSystem.TotalPhysicalMemory`
   - 系统：`Win32_OperatingSystem.Caption/Version/BuildNumber`
   - 显示器与刷新率可选（`Win32_VideoController.CurrentRefreshRate`）
   - 游戏 exe 的版本资源（`GetFileVersionInfo`）与文件大小
2. 脱敏：把 `C:\Users\<name>` 替换成 `%USERPROFILE%`，把 SteamID64（17 位数字）打码；只处理文本类文件（`*.ini` / `*.json` / `*.log` / `system.txt`），二进制原样。
3. 诊断包结构增加 `manifest.json`（工具版本、各文件 sha256、脱敏规则版本），方便对方核对。
4. 产出后给出「复制路径 / 打开目录 / 复制并开 Issue」三个出口（部分已有）。

**涉及**：`logs.rs` 的 `diag_bundle`；新增 `sysinfo.rs`（避免引 crate，用 PowerShell + 注册表）；`Logs.tsx` 出口按钮。

**验收**：诊断包内能看到显卡型号与驱动版本；用含中文用户名的路径测试，包内不出现该用户名；manifest 哈希与文件一致。

**风险**：WMI 在某些精简系统上不可用 → 失败要降级为「未采集到」，不能让整个诊断包失败。

---

### P1-1 代码签名

**问题**：安装包未签名，SmartScreen 显示「未知发布者」，普通用户到这一步就走一半。

**三条路（需要你选一条）**

| 方案 | 成本 | 适用 | 备注 |
|---|---|---|---|
| **SignPath Foundation** | 免费（开源项目） | 最推荐 | 需要项目公开、走他们的 GitHub App；构建在 CI 里做签名；有排队延迟 |
| **Certum Open Source** | €60–150/年 | OSS 可申请折扣 | 需要身份材料；给的是 OV/CS 证书，信誉积累靠下载量 |
| **Azure Trusted Signing** | 约 $10/月 | 个人/小团队 | 需要满足资格（组织历史或个人身份），审批后签名在 CI 里完成 |

> 不建议自签：对公网分发没有任何作用，仍会被 SmartScreen 拦。

**落地要点**

1. 证书/签名服务的私钥**只放 CI secrets**，不进仓库。
2. `tauri.conf.json` 的 `bundle.windows` 增加签名配置（`certificateThumbprint` 或 `signCommand` 指向 `signtool`），保留 `timestampUrl`（RFC3161 时间戳，签名过期后仍有效）。
3. NSIS 安装包与主程序**都要签**（Tauri 会分别签；若用 `signCommand` 要确保两者都过）。
4. 发布说明里附 `SHA256SUMS` 与签名指纹，方便用户自证。

**验收**：`signtool verify /pa /v` 通过；在干净 Win11 上双击不再弹「未知发布者」（可能有 SmartScreen 信誉积累期，属正常）；签名时间戳有效。

**风险**：签名服务审批周期不可控（SignPath 可能要几天到几周）→ 这条是**唯一有外部阻塞**的项，尽早发起申请，其余项并行。

---

### P0-2 GitHub Actions 发布流水线

**问题**：现在发布是「本机 build → 手工上传」，不可复现、换机器就断。

**方案**：`.github/workflows/release.yml`

```yaml
name: release
on:
  push:
    tags: ['v*']
  workflow_dispatch:
jobs:
  build:
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22, cache: npm, cache-dependency-path: app/package-lock.json }
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
        with: { workspaces: app/src-tauri }
      - run: cd app; npm ci
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # 签名相关 secrets 视方案而定
        with:
          tagName: ${{ github.ref_name }}
          releaseName: ${{ github.ref_name }}
          releaseDraft: true
          prerelease: false
          args: --bundles nsis
```

另外加一条 `ci.yml`：PR / push 到 main 时跑 `cargo test --lib` + `tsc --noEmit` + `vite build`（这三条现在都是人工跑，v0.3 已验证可通过）。

**验收**：推一个 `v0.4.0-rc.1` 标签，CI 自动产出草稿 Release，资产包含 NSIS 安装包 + `.sig`（更新用）+ `latest.json`；草稿里手动点发布后应用能检测到更新。

**风险**：CI 上 `cargo test` 里写 `%TEMP%` 的用例不受影响（CI 的 TEMP 可写）；但本地受限环境仍会挂，建议顺手换成 `tempfile`。

---

### P1-3 自动更新

**问题**：用户永远停在旧版。

**方案**

1. 依赖：`tauri-plugin-updater = "2"`、`tauri-plugin-process = "2"`（重启用）。
2. 生成更新签名密钥（与 Authenticode 是两套东西）：`tauri signer generate`，公钥写进 `tauri.conf.json` 的 `plugins.updater.pubkey`，私钥进 CI secrets。
3. `plugins.updater.endpoints` 指向 GitHub Releases 的 `latest.json`（`tauri-action` 会在 Release 里生成）。国内网络问题用现有的镜像/代理设置兜底（`net.rs` 已有系统代理读取）。
4. 前端：
   - 启动后 3 秒静默检查一次；有新版在设置→关于里显示「可用 0.4.0」，不打断用户。
   - 设置里给三种模式：`自动检查 / 仅提示 / 关闭`（默认仅提示）。
   - 下载走应用内进度（复用任务中心），完成后提示「重启并安装」。
5. 失败兜底：验签失败、下载中断、安装失败都要有明确文案 + 保留旧版（NSIS per-user 覆盖安装失败时不应破坏现有安装）。

**验收**：`0.3.0 → 0.4.0` 应用内升级成功且验签通过；断网时静默失败不报错弹窗；手动改 `latest.json` 版本号但不改签名 → 被拒绝。

**风险**：GitHub 在国内不稳 → 更新源要能切换（复用 `mirror` 设置）；README 里要写清「更新包同样会验签」。

---

### P1-5 日志实时跟随

**问题**：`get_log` 一次读全文再取尾 400 行（`logs.rs:8`），日志到几十 MB 时既慢又占内存；且要手动点刷新。

**方案**

1. 后端新增 `read_log_since(id, offset) -> { text, next, size, reset }`：`seek(offset)` 只读增量，单次上限 256 KB；文件变小（被截断/轮转）时返回 `reset=true` 让前端清空重来。
2. 前端 `Logs.tsx`：进入页面每 1 秒拉一次增量；「跟随」开关（滚动到底部时自动跟随，用户往上滚就暂停）；`visibilitychange` 隐藏时停轮询。
3. 保留手动「重新读取」按钮（重置 offset）。

**涉及**：`logs.rs`、`lib.rs`（`get_log` 改为增量接口，保留兼容的 `tail` 参数）、`store.ts`（`logOffset`/`logFollow`）、`Logs.tsx`。

**验收**：游戏在跑时日志每秒追加，界面 1 秒内跟上；50 MB 日志打开不卡；切到别的页面停止轮询。

**风险**：`nr_fg-*.log` 在切换游戏后文件名会变（`newest_log` 逻辑）→ 增量接口要检测「路径变了」并 reset。

---

### P1-6 帧数 / 延迟统计

**问题**：调参页改完参数，用户没有任何客观反馈；「开了更流畅」全靠感觉。

**前置**：**需要一份真实 `nr_fg-*.log` 样本**（我这台机器上没有装了 ANR 的游戏，无法确认日志格式）。拿到样本前不要写解析器。

**方案**

1. 新增 `fps.rs`：按行正则抽取图形层能确认的字段（等待样本后定），产出 `{ avgFps, p1Fps, frameTimeMs, framesGenerated, durationSec, samples[] }`。
2. `get_log_stats(id)` 返回最近一次会话的统计；调参页顶部加一条「上次会话：平均 78 fps · 1% low 52 · 生成帧占比 41%」。
3. 可选：把统计结果写进 `settings.json` 的 per-game 历史（最多 20 条），用于「换完参数对比」。**对比功能放到 v1.0**，v0.4 只做「看得到数字」。
4. 完全没有日志（游戏没跑过）时显示「还没有数据，进游戏跑一次」。

**验收**：对一份真实日志，解析出的帧数与该日志内自报值一致（允许 ±2%）；无日志时优雅空态；解析失败不崩（展示「日志格式未识别」）。

**风险**：日志格式随 ANR 版本变化 → 解析器要版本化（按 `nr_fg` 版本或日志头判断），无法识别时降级为原文展示。

---

### P2-10 单实例

**问题**：双开应用会同时写 `settings.json`、同时跑 `rescan`、甚至可以同时对同一个游戏执行安装（`jobs` 闸门是进程内的）。

**方案**：`tauri-plugin-single-instance`，第二个实例启动时聚焦第一个窗口并退出。

**验收**：双击两次只有一个窗口；第二个进程退出码 0。

**风险**：与托盘「关闭到托盘」的语义要一致（隐藏状态下启动第二个实例应当把窗口显示出来，复用 `show_main`）。

---

## 3. 里程碑建议

| 里程碑 | 包含 | 出口标准 |
|---|---|---|
| **M1 安全底座**（约 6 人日） | 4 崩溃捕获 → 9 CSP/外链 → 8 代理 DLL | 手炸应用有报告；CSP 不白屏；代理可切换并回滚 |
| **M2 可诊断**（约 7 人日） | 7 诊断包升级 → 5 日志跟随 → 6 帧数统计（拿到样本后） | 一份诊断包含显卡/驱动且脱敏；日志 1 秒跟随；帧数有数字 |
| **M3 可信分发**（约 9–12 人日，含外部审批） | 1 签名 → 2 CI → 3 自动更新 | 应用内 0.3→0.4 升级成功且验签；安装包签名有效 |
| **M4 收尾**（约 0.5 人日） | 10 单实例 + 文档/README 更新 | 双开被阻止；发布说明含 SHA256SUMS |

**并行提示**：M3 的证书申请**第一天就发起**（审批等待期与 M1/M2 并行），否则会成为关键路径。

---

## 4. 发布前验收清单

- [ ] `cargo test --lib` 全绿（含新增 crash / fps / sysinfo 用例）
- [ ] `cargo build --release` 0 warning
- [ ] `tsc --noEmit` 通过、`vite build` 通过
- [ ] `signtool verify /pa /v` 通过（安装包 + 主程序）
- [ ] 干净 Win10 与 Win11 各装一次，安装/卸载无残留
- [ ] 0.3.0 → 0.4.0 应用内升级通过；篡改 `latest.json` 被拒
- [ ] 诊断包含 GPU/驱动、无用户名；manifest 哈希一致
- [ ] 断网启动：不崩、不弹错误、更新检查静默失败
- [ ] 游戏运行中：所有写操作被拒并有解释
- [ ] 发布资产含 `ANR-Studio_0.4.0_x64-setup.exe`、`.sig`、`latest.json`、`SHA256SUMS`

---

## 5. 测试计划

**自动化（新增）**

- `crash.rs`：panic hook 落盘内容包含 location 与版本；ack 后不再提示
- `fps.rs`：3–5 个固定日志片段 → 期望统计值（拿到样本后写）
- `logs.rs`：`read_log_since` 的增量/截断/reset 三种路径（临时文件，用 `tempfile` 避免 `%TEMP%` 硬编码）
- `sysinfo.rs`：脱敏函数单测（用户名替换、17 位数字打码）

**手工**

- 签名 + SmartScreen 在干净系统验证（至少一台没装过本应用的机器）
- 更新链路：旧版 → 新版，以及「下载中断后重试」
- 代理 DLL 三种取值各切一次，看 `nr_fg_install.json` 与游戏是否正常
- 日志跟随：一边在游戏里改动设置（游戏写 ini、写日志）一边看界面

---

## 6. 需要你拍板的问题

1. **签名走哪条路**：SignPath Foundation（免费但要审批 + CI 集成）／Certum OSS（付费但自主）／Azure Trusted Signing（需要资格）？—— 这条决定 M3 的排期。
2. **更新源**：直接用 GitHub Releases 的 `latest.json`，还是同时准备一个国内可访问的静态地址（对象存储）作为兜底？
3. **隐私**：诊断包默认「含日志原文（已脱敏）」，是否改成「默认只含 system.txt + 错误行，用户勾选后才含完整日志」？
4. **CSP 取舍**：接受 `script-src 'unsafe-inline'`（保留单文件打包），还是放弃单文件换取更严的 hash 白名单？
5. **v0.4 是否包含「配置导入导出 / 预设档」**：我把它放在 v1.0，但如果它对你更重要，可以换掉「帧数统计」先做它。

---

## 7. v0.4 明确不做

- 注入 / 覆盖层 / 游戏内菜单替换（原有边界，不碰）
- Linux / Proton 支持（v1.0 再议）
- nr-record、ANR SR / RR（上游未稳定）
- 云端账号体系、配置同步
- 收藏/标签/排序、多语言扩语种（v1.0）

---

## 8. 开工第一步（给未来的自己）

1. 拉最新 `main`，确认 `git log` 顶部是 v0.3.0（`86928d1`）之后的提交。
2. **立刻发起签名服务申请**（对应第 6 节问题 1），拿到审批前先做 M1。
3. 建 `.github/workflows/ci.yml`，把现有三条命令接进 CI（先不谈签名与发布）。
4. 按 M1 顺序动工：`crash.rs` → CSP/外链 → 代理 DLL 覆盖。
5. 每完成一项，在设计文档追加一节（沿用附录字母序，从 **附录 T** 开始）。