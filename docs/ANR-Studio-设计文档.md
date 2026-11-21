# ANR Studio 设计文档

> 版本 v0.1 ｜ 日期 2026-11-14 ｜ 状态：待评审
> 对象：ANR（Apex Neural Rendering）的非官方图形化管理工具

---

## 0. 决策摘要

| 项目 | 决策 | 说明 |
|---|---|---|
| 技术栈 | **Tauri 2**（Rust 内核 + React 界面） | 单文件约 10MB，原生窗口/托盘/系统对话框 |
| 安装引擎 | **调用官方 `install.ps1` / `uninstall.ps1`** | 不重写安装逻辑，行为与官方完全一致 |
| 首版范围 | **核心版**：游戏库总览 / 安装卸载 / 版本切换 / 开关 / 参数 / 日志 | 不含 nr-record 与 SR/RR |
| 游戏发现 | **自动扫描（Steam/Epic/GOG）+ 手动添加** | 首次启动即可看到真实列表 |
| 平台 | Windows 10/11 x64 优先 | Linux/Proton 列为 v2 |

---

## 1. 背景与定位

### 1.1 现状

ANR 的官方交付方式是一个压缩包加两个脚本：

```
install.cmd "<游戏exe目录>" [dxgi.dll|winmm.dll|version.dll]
uninstall.cmd "<游戏exe目录>"
```

脚本本身写得很严谨（进程占用检查、备份、SHA256 校验、代理 DLL 自动判定、参数保留），但**全部信息都只存在于文件系统里，用户看不见**。由此产生四类真实痛点：

| 痛点 | 用户的实际情况 |
|---|---|
| 装过哪些游戏 | 只能靠自己记得，或者去翻每个游戏的 `OptiScaler` 目录 |
| 装的是哪个版本 | DLL **没有版本资源**，右键属性是空的，无从判断 |
| 现在开没开 | 得进游戏、开菜单、看 OptiScaler 状态才知道 |
| 换版本 / 卸载 | 回到命令行；卸载还会连带删掉整个 `OptiScaler` 目录 |

### 1.2 产品定位

> **一句话：让用户永远知道「哪些游戏装了 ANR、装的是哪一版、现在是开还是关」，并且一键就能改。**

ANR Studio 不是"给命令行套个壳"，而是一个**状态面板 + 操作中枢**：

- **看得见**：游戏库卡片直接呈现安装状态、版本、开关、是否有更新
- **点得动**：安装、切版本、开关、卸载，全部单次点击完成
- **不会坏**：复用官方脚本 + 操作前快照，任何失败都能回到原状

### 1.3 非目标（v1 明确不做）

- ✗ nr-record 的录制、回放、对比查看器（独立产品级体量，另立项目）
- ✗ ANR SR / ANR RR（官方尚未发布 beta）
- ✗ 管理其他 OptiScaler provider（Nukems、Arturs 等）
- ✗ 静默自动更新（改为提示 + 一键更新）
- ✗ 任何形式的游戏注入、内存读写、反作弊规避

### 1.4 命名与合规

- 名称：**ANR Studio**，界面与安装包内显著位置标注「非官方工具，与 Apex Compute 无关联」
- 不分发 ANR 二进制，仅在用户确认后从官方 GitHub Release 下载
- OptiScaler 为 GPL-3.0，本工具不修改其代码，只调用官方安装脚本

---

## 2. 技术可行性结论

### 2.1 已实测验证的事实

| 项 | 验证结果 | 依据 |
|---|---|---|
| 安装器可非交互调用 | ✅ 全参数化，仅缺参时才 `Read-Host` | `install.ps1` 63 行全文 |
| 安装状态可读 | ✅ `OptiScaler/nr_fg_install.json` 含 proxy / 原文件哈希 / 安装哈希 / 时间 | 同上 |
| 版本可识别 | ✅ `nr_fg_package.json` 的 `provider_sha256` 与 `nvngx_dlssg.dll` 的 SHA256 **完全一致**（实测 `ae7424b8…`） | 实测比对 |
| 开关可读写 | ✅ `OptiScaler.ini [FrameGen] Enabled=true/false` | 官方 ini 注释 |
| 代理 DLL 判定 | ✅ 官方已实现（Vulkan 游戏自动用 `winmm.dll`） | 脚本内正则扫描 exe |
| 卸载可还原 | ✅ 按 SHA256 校验后从 `nr_fg_backup` 还原 | `uninstall.ps1` |
| 安装类型可区分 | ✅ full 记录含 `proxy` 字段；addon 记录含 `optiscaler_ini`/`before` 字段 | 两包脚本对比 |
| 配置可保留 | ✅ 重装时保留用户自己的 `dlssg/nr_fg.ini` | `$keep` 逻辑 |

### 2.2 结论

**核心版技术可行，风险可控。** 三个套壳类工具最容易翻车的点（状态不可读、安装器需交互、版本不可识别）在本项目全部不存在。

预估工作量（单人）：

| 阶段 | 内容 | 工期 |
|---|---|---|
| M0 骨架 | Tauri 工程 + 设计系统 + 假数据界面 | 2 天 |
| M1 扫描 | 游戏库发现 + 状态解析 | 3 天 |
| M2 安装引擎 | 下载/缓存/校验/调用脚本/队列 | 4 天 |
| M3 版本与开关 | 切换、回滚、开关、参数、日志 | 3 天 |
| M4 打磨 | 主题、命令面板、托盘、诊断包、打包 | 3 天 |
| 合计 | 可发布 v1.0 | **约 3 周** |

---

## 3. 系统架构

### 3.1 进程与分层

```
┌─────────────────────────────────────────────────────────┐
│  WebView2（前端）                                        │
│  React 19 + TypeScript + Tailwind + shadcn/ui + Zustand │
└───────────────────────┬─────────────────────────────────┘
                        │ Tauri IPC（强类型 command）
┌───────────────────────┴─────────────────────────────────┐
│  Rust 内核                                               │
│  ┌────────────┬────────────┬────────────┬─────────────┐  │
│  │ scanner    │ installer  │ version    │ ini         │  │
│  │ 库扫描     │ 安装编排   │ 版本目录   │ 原子读写    │  │
│  ├────────────┼────────────┼────────────┼─────────────┤  │
│  │ net        │ proc       │ log        │ store       │  │
│  │ 下载/校验  │ 进程检测   │ 日志解析   │ 本地设置    │  │
│  └────────────┴────────────┴────────────┴─────────────┘  │
└───────────────────────┬─────────────────────────────────┘
                        │
        ┌───────────────┼────────────────┐
        ▼               ▼                ▼
   GitHub API      游戏目录          %LOCALAPPDATA%
   Releases        OptiScaler/*      ANR Studio/*
```

### 3.2 关键设计决策

| # | 决策 | 理由 | 代价 |
|---|---|---|---|
| D1 | 安装走官方脚本 `powershell -File install.ps1 "<dir>" [proxy]` | 官方升级自动兼容；边界情况已由官方处理 | 需依赖包内脚本路径稳定 |
| D2 | 包下载后缓存在本地，跨游戏复用 | 10MB 包装 20 个游戏只下 1 次 | 需缓存清理 UI |
| D3 | 自动判定 full 还是 addon | 用户不需要理解两个包的区别 | 判定逻辑需覆盖异常态 |
| D4 | 所有 ini 写入采用「临时文件 + 替换」原子写，保留 BOM/CRLF | 避免写坏配置导致游戏起不来 | 实现稍复杂 |
| D5 | 卸载前强制生成快照 zip | 官方卸载会删整个 `OptiScaler` 目录 | 占少量磁盘 |
| D6 | 不内置 chromium，用系统 WebView2 | 体积 10MB vs 150MB | 依赖 Win10+ 自带 WebView2 |

### 3.3 IPC 命令清单

| 命令 | 入参 | 出参 | 说明 |
|---|---|---|---|
| `scan_library` | `{}` | `Game[]` | 全量扫描，带进度事件 |
| `add_game_manual` | `{ path }` | `Game` | 校验含 `.exe`，去重 |
| `remove_game` | `{ id }` | `void` | 仅移出列表，不动文件 |
| `install` | `{ id, version? }` | `TaskId` | 进队列，返回任务号 |
| `uninstall` | `{ id }` | `TaskId` | 含快照 |
| `switch_version` | `{ id, version }` | `TaskId` | 重装保配置 |
| `set_fg_enabled` | `{ id, enabled }` | `void` | 写 `[FrameGen] Enabled` |
| `read_provider_config` | `{ id }` | `NrFgConfig` | 读 `nr_fg.ini` |
| `write_provider_config` | `{ id, patch }` | `void` | 写 `nr_fg.ini` |
| `list_releases` | `{ refresh? }` | `Version[]` | GitHub API + 本地缓存 |
| `prefetch_package` | `{ version, kind }` | `TaskId` | 预下载到缓存 |
| `get_logs` | `{ id, tail? }` | `LogChunk` | 读最新 `nr_fg-*.log` |
| `make_diag_bundle` | `{ id }` | `Path` | 打包日志+ini+系统信息 |
| `get_settings` / `set_settings` | — | `Settings` | 主题、镜像、缓存目录 |
| `cancel_task` | `{ taskId }` | `void` | 取消下载/安装 |

事件（后端 → 前端）：`task://progress`、`task://log`、`scan://progress`、`game://changed`。

---

## 4. 数据模型

```rust
// 游戏库条目
struct Game {
    id: String,              // 规范化路径的 hash
    name: String,            // 优先取平台元数据名，回退 exe 主名
    exe_path: PathBuf,
    install_dir: PathBuf,
    source: GameSource,      // Steam / Epic / Gog / Manual
    cover: Option<PathBuf>,  // 本地缓存的封面
    status: GameStatus,
    anr: Option<AnrState>,   // None = 未安装
}

enum GameSource { Steam { app_id: u32 }, Epic, Gog, Manual }

enum GameStatus {
    NotInstalled,
    Installed,
    InstalledOutdated { latest: String },
    ThirdPartyOptiScaler,     // 有 OptiScaler 但不是我们的
    Conflict { reason: String },
    GameRunning,              // 运行时叠加态
}

struct AnrState {
    kind: PackageKind,        // Full | AddOn
    version: Option<String>,  // 由 provider DLL 指纹反查，可能为 Unknown
    proxy_dll: String,        // dxgi.dll / winmm.dll / version.dll
    installed_at: DateTime<Utc>,
    fg_enabled: bool,         // [FrameGen] Enabled
    fg_input: String,         // nvngxfg / dlssg / ...
    config: NrFgConfig,
    log_path: Option<PathBuf>,
    health: Health,           // Ok | Warn(Vec<String>) | Error(String)
}

enum PackageKind { Full, AddOn }

struct NrFgConfig {           // 映射 dlssg/nr_fg.ini
    path: String,             // auto | coopmat | packed | dp4a | portable ...
    model_size: String,
    nits_per_unit: u32,
    max_generated: u8,
    scene_cut: bool,
}

struct Version {
    tag: String,              // FG-V.B15
    product: String,          // ANR FG Beta
    kind: PackageKind,
    published_at: DateTime<Utc>,
    provider_sha256: String,  // 版本指纹
    size: u64,
    notes_url: String,
    is_latest: bool,
    cached: bool,             // 本地已缓存
}
```

**版本识别机制**：安装状态里没有版本号，因此维护一张指纹表 `{ provider_sha256 → 版本 }`，来源为各 Release 资产内的 `nr_fg_package.json`（该文件不在安装目录内，需在下载时抽取并缓存）。未命中则显示「未知版本（可识别为安装时间：xxxx）」，并允许用户手动标记。

---

## 5. 核心流程

### 5.1 状态判定矩阵（设计的核心）

所有界面状态由这张表驱动，避免"猜"：

| `OptiScaler/` | `OptiScaler.ini` | `nr_fg_install.json` | provider 目录 | 判定 | 主操作 |
|---|:--:|:--:|:--:|---|---|
| 无 | 无 | 无 | 无 | 未安装（干净目录） | **全量安装** |
| 有 | 有 | 无 | 无 | 第三方 OptiScaler | **插件安装**（addon） |
| 有 | 有 | 有 `proxy` | 有 | 本工具全量安装 | 管理 / 切换 / 卸载 |
| 有 | 有 | 有 `optiscaler_ini` | 有 | 本工具插件安装 | 管理 / 切换 / 移除 |
| 有 | 有 | 无 | 有 | 未知来源残留 | ⚠️ 需人工确认后再操作 |
| 有 | 无 | 有 | 有 | 异常（ini 丢失） | ⚠️ 修复建议 |
| 无 | 有 | — | — | 异常 | ⚠️ 手动处理 |

> 判定顺序严格按上表，与官方两个脚本的前置检查一一对应，保证"界面显示的"与"脚本会做的"永远一致。

### 5.2 安装流程

```
用户点击 [安装]
   │
   ├─ 1. 前置检查（全部通过才继续，任一失败给出可操作提示）
   │     · 目录存在且含 .exe
   │     · 游戏进程未运行（扫描进程 Path 前缀）
   │     · 目录可写（试写临时文件）
   │     · VC++ 2015-2022 x64 运行时是否齐全（MSVCP140.dll 等 4 个）
   │     · 磁盘剩余空间 ≥ 100MB
   │     · 反作弊风险提示（命中已知列表时弹确认，非阻断）
   │     · 状态机判定安装类型（5.1）
   │
   ├─ 2. 取得安装包
   │     · 缓存命中 → 直接用
   │     · 未命中 → 下载（镜像可选 + 断点续传 + 进度条）
   │     · 校验 SHA256（对照 SHA256SUMS.txt）
   │     · 解压到 %LOCALAPPDATA%\ANR Studio\cache\<version>\<kind>\
   │
   ├─ 3. 执行官方脚本
   │     powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1 "<dir>" [proxy]
   │     stdout/stderr 实时流式显示在任务面板
   │
   ├─ 4. 后置校验
   │     · nr_fg_install.json 存在
   │     · provider 目录内 nvngx_dlssg.dll 存在
   │     · 指纹比对确定版本
   │
   └─ 5. 刷新卡片状态 + 轻提示（不打断的 toast）
```

**失败处理**：脚本非零退出时，展示原始输出 + 一句人话解释 + 三个选项：重试 / 查看诊断包 / 回滚（若已有安装记录）。

### 5.3 版本切换

官方重装逻辑会保留用户的 `nr_fg.ini`，因此切换 = 用目标版本再跑一次安装：

1. 下载目标版本包（若未缓存）
2. 记录当前版本（用于回滚）
3. 调 `install.ps1` 覆盖安装（`nr_fg.ini` 自动保留）
4. 指纹校验 → 更新卡片

界面上呈现为「版本」下拉 + 「回滚到上一版」按钮，不出现任何命令。

### 5.4 卸载流程

官方 `uninstall.ps1` 末尾是 `Remove-Item -Recurse -Force $opti`，**会删除整个 `OptiScaler` 目录**。因此：

1. 卸载前**强制**生成快照：`%LOCALAPPDATA%\ANR Studio\snapshots\<game>-<time>.zip`（含整个 `OptiScaler` 目录）
2. 二次确认弹窗明确写出：「将删除该游戏的 OptiScaler 目录，已自动备份到 …」
3. 若检测到目录内有其他 provider（`custom/` 下除 ANR 外还有子目录）→ **额外红色警告**，列出将被删除的项
4. 执行卸载 → 校验代理 DLL 已还原 → 提示「可从快照恢复」

---

## 6. UI / UX 设计

### 6.1 设计原则

1. **状态优先**：卡片第一眼给出状态，操作按钮第二眼才出现
2. **单点操作**：主操作永远只有一个主色按钮，其余降为次级
3. **不做弹窗工厂**：安装等长任务进任务面板，不阻塞界面
4. **深色为默认**：游戏场景下的第一选择；浅色完整对齐，非降级
5. **扁平无阴影**：用 1px 边框和层级色区分，不用投影和渐变堆叠高级感

### 6.2 设计 Token

**色彩**

| 语义 | 深色 | 浅色 |
|---|---|---|
| 页面底 | `#0B0C0F` | `#FAFAFB` |
| 卡片面 | `#14161A` | `#FFFFFF` |
| 次级面 | `#1B1E23` | `#F4F5F7` |
| 描边 | `#262A31` | `#E7E8EA` |
| 主文字 | `#E6E8EC` | `#17181A` |
| 次文字 | `#8B919C` | `#6B7280` |
| 强调色 | `#5B7CFA` | `#4F6EF7` |
| 成功（已启用） | `#3FB984` | `#1E9E6A` |
| 警告（需注意） | `#D9A03C` | `#B87B12` |
| 危险（失败） | `#E2555A` | `#C9383D` |

强调色只用于主操作与选中态，全屏面积占比 < 5%，这是"高级感"的关键约束。

**字体与尺度**

| 项 | 值 |
|---|---|
| 中文 | MiSans / HarmonyOS Sans / 微软雅黑 UI（逐级回退） |
| 西文 | Inter |
| 等宽（路径/哈希） | JetBrains Mono |
| 字号 | 11 / 12 / 13 / 15 / 20 / 28 |
| 圆角 | 控件 8 · 卡片 14 · 弹层 16 |
| 间距 | 4 的倍数，卡片内边距 20 |
| 动效 | 120–180ms `ease-out`，仅做透明与位移，不用弹跳 |

**组件库**：React 19 + Vite + TypeScript + Tailwind CSS v4 + **shadcn/ui**（Radix 无样式内核，可完全按上面 token 重绘）+ lucide-react 图标 + Zustand 状态。

> 选 shadcn/ui 而非 MUI/AntD 的理由：后两者自带强视觉主张，改造成"扁平简约"要对抗其设计语言；shadcn 提供的是可访问性内核 + 源码，视觉完全由 token 决定。

### 6.3 信息架构

```
侧边栏（80px 图标 + 文字）
├─ 总览      ← 默认页
├─ 游戏库
├─ 版本
├─ 日志诊断
└─ 设置
```

### 6.4 页面设计

#### ① 总览

```
┌──────────────────────────────────────────────────────────────┐
│  ANR Studio                        [搜索]  [Ctrl K]  [☾]     │
├──────────────────────────────────────────────────────────────┤
│  已安装 12 个游戏 · 当前 V.B15 · 3 个可更新                    │
│  ┌────────────┬────────────┬────────────┬────────────┐       │
│  │ 12 已启用  │ 2 已关闭   │ 1 需注意   │ V.B15 最新 │       │
│  └────────────┴────────────┴────────────┴────────────┘       │
│                                                              │
│  最近使用                              [全部游戏 →]           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ 封面      │ │ 封面      │ │ 封面      │ │ + 添加   │        │
│  │ 游戏名    │ │ 游戏名    │ │ 游戏名    │ │  游戏    │        │
│  │ ●已启用   │ │ ○已关闭   │ │ ⚠需注意  │ │          │        │
│  │ V.B15 ▾  │ │ V.B13 ▾  │ │ —        │ │          │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
└──────────────────────────────────────────────────────────────┘
```

四格统计条是"总览"的全部意义：**一眼回答"我装了什么"**。

#### ② 游戏库

- 网格 / 列表双视图（记住选择）
- 筛选：全部 / 已安装 / 未安装 / 有更新 / 需注意
- 搜索按名称模糊匹配
- 卡片悬停浮出快捷操作：安装、开关、详情、打开目录
- 多选后底部浮出操作条：批量安装、批量开关、批量卸载
- 从资源管理器拖入文件夹到窗口 = 添加游戏

#### ③ 游戏详情（右侧抽屉，宽 480，不跳页）

```
游戏名                              [关闭]
路径 …\steamapps\common\XXX    [打开]
─────────────────────────────────────
状态      ● 已启用   V.B15
安装方式  全量（含 OptiScaler）
代理 DLL  dxgi.dll
安装时间  2026-11-13 21:04
─────────────────────────────────────
帧生成          [ 开 ]
版本            [ V.B15 ▾ ]
高级参数        ▸ 折叠
  · 卷积路径    auto ▾
  · 最大生成帧  5
  · 场景切换保护 开
  · HDR 亮度    80 nits
─────────────────────────────────────
[查看日志]  [生成诊断包]   [卸载]
```

抽屉而非独立页面：用户改完立刻看到卡片状态变化，不丢失上下文。

#### ④ 版本

- 版本卡片列表（当前版本高亮，指纹命中显示「已安装于 3 个游戏」）
- 每个版本：发布日期、体积、是否已缓存、操作（下载 / 设为默认 / 查看说明）
- 底部：缓存占用 + 一键清理

#### ⑤ 日志诊断

- 左侧游戏列表，右侧日志尾部（等宽字体，错误行红标）
- 「生成诊断包」→ 输出 zip：`nr_fg-*.log` + `OptiScaler.log` + 两个 ini + 显卡/驱动/系统信息 + 安装记录
- 直接给出「复制到剪贴板并打开 GitHub Issue 页」

### 6.5 便利性设计清单

1. `Ctrl+K` 全局命令面板：搜索游戏、执行操作、跳转页面
2. 拖拽文件夹到窗口即添加游戏
3. 记住每个游戏的最近操作时间，「最近使用」自动排序
4. 安装进队列，串行执行（避免多个安装器同时写文件），可后台继续
5. 游戏运行中：卡片显示「运行中」并禁用危险操作，附一句解释
6. 路径一键复制 / 打开所在目录
7. 全局提示不打断：右下角 toast，3 秒消失，失败时留在原地
8. 首次启动引导：自动扫描 → 列出可安装游戏 → 一键全部安装
9. 主题跟随系统，也可手动锁定
10. 关闭窗口最小化到托盘，托盘菜单显示「已安装 12 · 已启用 10」
11. 卡片右键菜单：安装/卸载/切版本/复制路径/在资源管理器中显示
12. 键盘可达：Tab 顺序合理，所有操作有焦点环
13. 失败可回滚：任何安装失败的下一步都是「回滚」而非「重装」
14. 无网络时优雅降级：本地状态照常显示，只禁用需要下载的操作

### 6.6 动效与反馈

- 卡片状态色切换：120ms 颜色过渡
- 抽屉进出：180ms 位移 + 透明度
- 安装进度：任务面板内线性进度 + 阶段文字（检查 / 下载 / 解压 / 安装 / 校验）
- 不使用：骨架屏闪烁、弹跳、旋转大图标

---

## 7. 安全与风险

| # | 风险 | 影响 | 处理 |
|---|---|---|---|
| R1 | 卸载删除整个 `OptiScaler` 目录 | 用户其他 provider 丢失 | 卸载前强制快照 + 列出将删除项 + 二次确认 |
| R2 | 反作弊游戏注入代理 DLL | 账号封禁 | 命中已知反作弊名单时红色警告并要求勾选确认；不做任何规避 |
| R3 | ini 写入破坏格式（BOM/CRLF） | 游戏无法启动 | 原子写 + 保留原编码 + 写前 `.bak` |
| R4 | 游戏运行中安装导致文件占用 | 安装失败/游戏崩溃 | 进程 Path 前缀扫描，硬阻断 |
| R5 | GitHub 下载在国内失败 | 无法安装 | 镜像加速选项 + 断点续传 + 手动导入本地包 |
| R6 | 包被篡改 | 安全风险 | 强制 SHA256 校验，不匹配即拒绝，不允许跳过 |
| R7 | 指纹表未收录新版本 | 版本显示为未知 | 降级显示安装时间；允许手动拉取最新指纹 |
| R8 | 官方包结构变更 | 脚本路径失配 | 启动时自检包结构，失败则提示"官方包结构已变更，请更新本工具" |
| R9 | 用户把工具放进 Steam 库目录 | 被 Steam 校验覆盖 | 添加时检测 `steamapps` 路径，提示改用其他位置 |

---

## 8. 项目结构

```
anr-studio/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ commands/        # IPC 命令薄层
│  │  ├─ scanner/         # steam.rs epic.rs gog.rs manual.rs
│  │  ├─ installer/       # detect.rs download.rs run.rs verify.rs
│  │  ├─ version/         # catalog.rs fingerprint.rs
│  │  ├─ config/          # ini.rs（原子读写，保 BOM/CRLF）
│  │  ├─ log/             # parse.rs bundle.rs
│  │  ├─ proc/            # running.rs
│  │  └─ store/           # settings.rs snapshot.rs
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ src/
│  ├─ pages/              # Overview Library Versions Logs Settings
│  ├─ components/         # GameCard StatusDot TaskPanel Drawer ...
│  ├─ styles/tokens.css   # 设计 token
│  ├─ lib/ipc.ts          # 强类型 IPC 封装
│  └─ store/              # Zustand
├─ docs/
└─ package.json
```

---

## 9. 里程碑与验收

| 里程碑 | 交付物 | 验收标准 |
|---|---|---|
| **M0** | 可运行的 Tauri 骨架 + 设计系统 + 假数据总览/游戏库 | 深浅主题切换正常，视觉达到设计稿水准 |
| **M1** | 真实扫描 + 状态解析 | 在本机 Steam 库正确列出游戏并标出安装状态；无 Steam 时引导手动添加 |
| **M2** | 安装引擎全链路 | 对一个干净游戏目录完成安装，游戏内生效；重复安装保留 `nr_fg.ini` |
| **M3** | 版本切换 + 开关 + 参数 + 日志 | V.B15 ↔ 旧版本双向切换成功；开关写入后游戏行为符合预期 |
| **M4** | 打磨 + 打包 | 安装包 < 15MB，冷启动 < 1.5s，托盘/命令面板可用，卸载后可完美还原 |

**v1 发布门槛**：连续对 5 个不同类型的游戏目录（D3D12 / Vulkan / 已有第三方 OptiScaler / 已有本工具安装 / 含中文路径）完成"安装 → 切换版本 → 关闭 → 卸载 → 还原"全流程无残留。

---

## 10. 待验证 / 待定

| # | 问题 | 计划 |
|---|---|---|
| Q1 | `FGNvngxReplacement=auto` 是否等价于"旁路到原生 DLSS-G" | 需实测；确认后增加「用原生 DLSS-G 代替」三态开关 |
| Q2 | 指纹表如何自动更新 | 优先随工具版本内置；后续可从小型托管 JSON 拉取 |
| Q3 | Steam 封面图来源 | 优先 Steam CDN（`steamcdn-a.akamaihd.net/steam/apps/<id>/header.jpg`），需确认可用性 |
| Q4 | Epic/GOG 扫描覆盖率 | 按 Epic manifests（`*.item`）与 GOG 注册表实现，实测补漏 |
| Q5 | 是否需要 Linux/Proton | 需 `WINEDLLOVERRIDES` 改 Steam 启动项，涉及 `localconfig.vdf`，风险高，列 v2 |
| Q6 | 多语言 | v1 中文为主，文案抽离到 `i18n/zh-CN.ts`，预留英文 |

---

## 附：为什么不做"完全自研安装逻辑"

官方 `install.ps1` 里有若干不显眼但关键的边界处理：代理 DLL 的三档判定（含对 exe 的 UTF-16 字符串扫描）、备份哈希双向校验、旧目录名 `nrfg → Apex Neural Rendering FG` 的迁移、`nr_fg.ini` 的 `.keep` 保留机制、缺少 VC++ 运行时的检测。自研等价实现至少需要完整复刻这些分支，且官方每次发版都可能微调。**复用脚本、只做编排与校验，是投入产出比最高的选择。**

代价是必须接受一个约束：本工具的能力上限 = 官方脚本的能力上限。对于"管理多个游戏"这个目标，这个上限完全够用。

---

## 附录 B：实现对照（v0.1 已落地）

设计与实现基本一致，以下是落地时的偏离与原因：

| 项 | 原设计 | 实际实现 | 原因 |
|---|---|---|---|
| 安装 | 调用官方 `install.ps1` | ✅ 一致 | 下载→校验→解压→调脚本→后置校验 |
| 卸载 | 调用官方 `uninstall.ps1` | **改为 Rust 原生实现** | 卸载不依赖包缓存（用户可能已清缓存）；逻辑与官方等价：按哈希校验还原 → 删目录。addon 场景额外保护：只删 provider、把 ini 恢复成安装前的值，**不删用户的 OptiScaler** |
| 版本标签 | `V.B15` | `FG-V.B15`（GitHub 原始 tag） | 保证与 Release 资产名、目录名完全一致，避免"检测版本"与"版本列表"对不上 |
| 反作弊 | 命中名单时红色警告 | 未做名单，仅通用提示 | 名单维护成本高且容易误判，先留通用提示 |
| 镜像加速 | 可选镜像 | `gh-proxy.com` 前缀 | 仅在用户手动选择"镜像"时启用 |
| 目录选择 | 系统对话框插件 | PowerShell 调 `FolderBrowserDialog` | 少一个依赖，功能等价 |
| 缓存位置 | `%LOCALAPPDATA%\ANR Studio` | `%LOCALAPPDATA%\app.anrstudio.desktop\anr-studio` | Tauri 的 app_local_data_dir 约定 |

### 已实测验证的链路

用 `C:\Users\12558\ANR-TestGame`（一个只含 dummy exe 的假目录）跑完整流程：

1. 手动添加目录 → 卡片出现，状态「未安装」
2. 安装 → 下载 10.4MB 包 → SHA256 校验 → 解压 → 官方脚本执行 → 目录内出现 `dxgi.dll`(30MB)、`OptiScaler.ini`、`OptiScaler/nr_fg_install.json`
3. 版本识别 → 卡片显示 `FG-V.B15`（指纹 `ae7424b87d13…` 命中）
4. 关开关 → `[FrameGen] Enabled` 由 `auto` 改为 `false`，1557 行 ini 其余内容零改动
5. 卸载 → 快照 3.2MB 生成；`dxgi.dll`、`OptiScaler.ini`、`OptiScaler/`、`.anr-bak` 全部清除，目录只剩原始的 `TestGame.exe`
6. 二次安装（缓存命中）→ 8 秒内完成，无重复下载
7. 诊断包 → zip 内含 `OptiScaler.ini`、`nr_fg.ini`、`nr_fg_install.json`、`system.txt`


---

## 附录 C：自绘标题栏与界面密度

### C.1 为什么要自绘标题栏

系统默认标题栏会把窗口拉回"Windows 95 风格"：白底或黑底的一条，与应用内部的深色界面形成割裂，也无法控制字号与层级。自绘后，标题栏成为设计系统的一部分，深浅主题同步切换。

实现要点：

| 项 | 做法 |
|---|---|
| 关闭系统装饰 | `tauri.conf.json` → `windows[0].decorations: false` |
| 拖拽 | 标题栏元素加 `data-tauri-drag-region`（需 `core:window:allow-start-dragging`） |
| 窗口按钮 | `@tauri-apps/api/window` 的 `minimize` / `toggleMaximize` / `close`，对应三个 `core:window:allow-*` 权限 |
| 最大化状态 | `onResized` 里查 `isMaximized()`，按钮图标在「最大化 / 还原」间切换 |
| 双击最大化 | 拖拽区 `onDoubleClick` → `toggleMaximize`（Tauri 的拖拽区本身不处理双击） |
| 居中对齐 | 左右各留 138px（3 × 46px 按钮宽度）占位，中间区域自然居中，不依赖绝对定位 |
| 层级 | 标题栏 `z-index: 60`，高于所有弹层；同时把抽屉/面板的容器改为 `top: var(--titlebar-h)`，让弹层从标题栏下方开始，窗口控制始终可点 |
| 版本号 | 构建期由 Vite `define` 注入 `__APP_VERSION__`（读 package.json），无需运行时 IPC |

标题栏高 36px，窗口高度相应从 840 提到 876，保证内容区高度不变。

### C.2 界面密度怎么落地

难点在于：界面是用 Tailwind 的 px 写死的，密度切换不能靠"覆盖类名"。因此把与密度相关的尺寸抽成 CSS 变量，组件只引用变量：

| 变量 | 标准 | 紧凑 | 影响 |
|---|---|---|---|
| `--dens-pad-x / -y` | 32 / 26 | 18 / 14 | 页面四周留白 |
| `--dens-gap` | 12 | 8 | 网格间距 |
| `--dens-col` | 228px | 184px | 卡片最小宽度（决定每行几个） |
| `--dens-card-pad` | 12 | 8 | 卡片内边距 |
| `--dens-stat-pad` | 16 | 12 | 统计卡内边距 |
| `--dens-row-py` | 8 | 4 | 列表行上下留白 |
| `--dens-nav-h` | 34 | 30 | 侧边栏导航项高度 |
| `--dens-topbar` | 56 | 48 | 顶栏高度 |
| `--dens-side` | 214px | 196px | 侧边栏宽度 |

卡片网格改为 `grid-template-columns: repeat(auto-fill, minmax(var(--dens-col), 1fr))`，密度变化时列数自动调整（1280px 宽下：标准 4 列 → 紧凑 5 列），不需要为每档密度写断点。切换写在 `<html data-density="compact">` 上，`App` 从设置同步，设置项持久化到 `settings.json`。

---

## 附录 D：第二轮迭代（菜单 / 设置 / 封面 / 修复）

### D.1 标题栏多级菜单

左侧去掉了图标，换成「界面 / 工具 / 关于」三个菜单项，支持二级子菜单：

- 界面 → 主题（深色/浅色/跟随系统）、界面密度（标准/紧凑）、字体大小（五档）
- 工具 → 重新扫描、版本与缓存、日志诊断、打开数据目录
- 关于 → 软件说明（弹窗）、ANR 官方仓库

实现要点：菜单状态是扁平的两层结构（`Item { label, children?, checked?, onClick }`），子菜单在 hover 时渲染成嵌套的 `.menu-panel`，靠 CSS 的 `.menu-panel .menu-panel { left: calc(100% + 2px) }` 定位到右侧；点击外部或 Esc 关闭。这样加菜单项只改数据，不动渲染逻辑。

### D.2 设置页分类与字体缩放

设置页左侧是独立分类导航（外观/下载/安全/通用/关于），与主导航分离。

字体大小用五档滑杆，映射到整体 `zoom`：0.9 / 0.95 / 1 / 1.08 / 1.15。为什么用 `zoom` 而不是改根字号：界面尺寸几乎都是 px 写死的，改根字号对 px 无效；`zoom` 会让整个文档等比缩放并重新布局（列数会自动变化），是 WebView2 下唯一不用重写样式的方案。代价是缩放后逻辑视口变小，属于预期行为。

### D.3 封面自动抓取

没有 API key 的前提下，选了两个公开源：

| 源 | 接口 | 说明 |
|---|---|---|
| Steam 商店 | `store.steampowered.com/api/storesearch/?term=&cc=CN&l=english` | 返回 appid，再取 `cdn.cloudflare.steamstatic.com/steam/apps/<appid>/header.jpg`（460×215） |
| GOG | `gog.com/games/ajax/filtering/api/zh/games?search=` | 返回商品图，Steam 失败时回落 |

关键点：

1. **必须用 `l=english`**：中文界面返回的是「赛博朋克 2077」，和游戏目录名 `Cyberpunk 2077` 对不上，匹配会掉到 "Cyberpunk 2077 REDmod" 这种周边应用上（这是实测踩到的坑）
2. **必须开 reqwest 的压缩特性**：Steam 会返回 gzip/br，`default-features=false` 且不开 `gzip`/`brotli`/`deflate` 时 JSON 解码直接失败
3. **匹配打分**：名称归一化后比长度差（精确 100 / 前缀 80 / 包含 60），并过滤 dlc、soundtrack、demo、mod、tool 等噪音条目
4. **启动时后台探测**选源，结果放 `OnceLock`；封面存 `%LOCALAPPDATA%\...\covers\<gameId>.jpg`，扫描时转 base64 随列表下发
5. 卡片右键菜单提供「重新搜索封面」：删缓存文件 → 重新抓 → 事件推回

> 封面存在应用数据目录而不是游戏目录：写进游戏文件夹会被 Steam 的文件校验当成异常文件，可能触发「修复」。

### D.4 修掉的两个 bug

| 问题 | 根因 | 修复 |
|---|---|---|
| 安装时步骤在动但进度条是 0 | 后端 `progress` 是 0..1 的小数，前端进度条按 0..100 渲染 | 推 event 前统一乘 100（前端也做了 <=1 的兜底归一化） |
| 两个任务同时下载同一个包，zip 互相覆盖 | `ensure_package` 没有并发保护 | 加 `static PKG_LOCK: Mutex<()>`，同一时刻只允许一个下载/解压 |


---

## 附录 E：托盘与关闭行为

关闭窗口时不让用户"一关就没了"，也不强行藏起来，而是询问一次并记住：

| 设置值 | 行为 |
|---|---|
| `ask`（默认） | 关闭被拦截，后端发 `close-requested` 事件，前端弹窗询问 |
| `tray` | 直接 `window.hide()`，程序和任务继续后台运行 |
| `exit` | 正常关闭，进程退出 |

实现要点：

1. `Builder::on_window_event` 里拦 `CloseRequested`：先读设置，`ask` 时 `api.prevent_close()` 并发事件；`tray` 时 prevent + hide；`exit` 时直接放行
2. 弹窗里选了"直接退出"要能真的关掉：用 `AppState.allow_close: AtomicBool` 置位后再 `window.close()`，第二次进入 `CloseRequested` 时看到标志位就放行（避免自己拦自己）
3. 托盘用 `TrayIconBuilder` + 菜单（显示主窗口 / 退出），左键点击 `show_main`（unminimize + show + focus），`show_menu_on_left_click(false)` 让左键不弹菜单
4. 弹窗里的"记住我的选择"由 `close_window(action, remember)` 一次性处理：先落盘设置再执行动作，避免前端先写设置、后端读到旧值的竞态
5. 前端选完后也要同步一次本地 `settings.closeAction`，否则设置页会显示旧值（这是实测抓到的）

---

## 附录 F：更新日志（0.1.1）

### F.1 数据从哪来

版本列表本来就随 `list_releases` 拉回了 release 的 `body`（Markdown），只是之前只截了一行当摘要。现在单独开一个 `get_changelog(tag)` 命令，按需解析：

1. 定位 `## Changes` 小节（兼容 `What's new` / `Changelog`），到下一个标题为止
2. 没有该小节时，退化为取 `## Packages` 之前的全部内容（并过滤掉过短的样板行）
3. 每行削掉 Markdown 标记（`[text](url)` → `text`、反引号、强调符号），缩进的续行并到上一条
4. `Packages` / `Install` / `Supported GPUs` / `Feedback` 这些每版都一样的小节直接跳过

### F.2 中文译文从哪来

ANR 的 release 正文只有英文（仓库里也没有中文文档），所以中文是**机器翻译**：

| 项 | 做法 |
|---|---|
| 接口 | MyMemory 公共接口（免 key）；实测 Google gtx、Lingva、LibreTranslate 在本机均不可达 |
| 粒度 | 逐条翻译，单条超过 420 字符按句子切块再拼回 |
| 缓存 | `<数据目录>/changelog/<tag>.json`，命中后 3ms 返回，不重复联网 |
| 失败 | 某条翻译失败就留空，界面只显示英文，不写缓存（下次展开重试） |
| 时效 | 首次展开约 4–6 秒（3–4 条 ≈ 6 次请求），之后走缓存瞬时打开 |

界面上中文在上、英文原文在下（浅色小字），底部标注「中文为自动翻译，仅供参考」；右上角有「GitHub 原页」直达 release 页面。取不到正文时显示「这个版本没有提供更新日志」。

### F.3 交互

箭头按钮在每行最左侧，默认收起；点击才发请求（不预取，避免打开页面就联网翻译十个版本）。箭头 90° 旋转动画，面板从行下方展开、底色比列表浅一档。

### F.4 设置弹窗固定高度

之前是 `max-h`（内容少就矮），现在是 `h-[min(640px,86vh)]`：高度恒定，内容少时下方留空，内容多时右侧内容区自己滚动，只有内容区滚动、左侧分类不动。

### F.5 顺带修掉的字段命名 bug

Rust 的 `#[serde(rename_all = "camelCase")]` 把 `size_mb` 转成 `sizeMb`、`third_party_optiscaler` 转成 `thirdPartyOptiscaler`，而前端读的是 `sizeMB` / `thirdPartyOptiScaler`：

- 版本列表的包体积一直是 `NaN MB`（显示成 `—`）
- **更严重**：`thirdPartyOptiScaler` 恒为 `undefined`，导致「这个游戏已经装了 OptiScaler，该走 addon 包」的判断失效，真实安装时会给这类游戏装 full 包

已用 `#[serde(rename = "...")]` 逐个钉死，实测带 `OptiScaler.ini` 的目录现在能正确返回 `thirdPartyOptiScaler: true`。

---

## 附录 G：0.1.2（改名 / 总览重做 / 图标缓存）

### G.1 游戏改名

右键卡片 →「修改游戏名」→ 输入框（预填当前名）→ 保存。实现要点：

- 自定义名存在 `settings.json` 的 `renames`：`{ 游戏 id: 显示名 }`，游戏 id 是目录路径的 FNV 哈希，目录不移动就一直有效
- `rescan()` 构建完每个 Game 后套一次自定义名，所以列表、抽屉、总览、封面搜索用的都是同一个名字
- `rename_game(id, name)` 一次调用完成：写设置 → 重新扫描 → 删掉旧封面 → 用新名字重抓 → emit `cover` 事件
- 名字传空等于「恢复原名」，设置里删掉键即可，不会残留

### G.2 总览页重做

原来的四个等宽统计卡（图标 + 数字 + 副标题）是最典型的后台模板，问题在于信息平权——四个数字都一样的重量，用户读完不知道先看哪个。改成：

1. **一个主指标 + 状态带**：左上「已启用 5/6」用 40px 数字，右边一条 6px 高的色块带，每个已装游戏一段，绿色=启用、灰色=关了；下面一行图例。信息变成"整体感受 + 可下钻"，不再是四个并列数字
2. **两栏非对称**：左栏 1fr 放「需要处理」「最近使用」，右栏 286px 放当前版本、包体积、缓存数、快捷操作。右侧栏在滚动时 sticky
3. **弱化容器**：列表用 `border-line-soft` 的浅边框 + 行分隔，不再每块都套 14px 圆角卡片；区块标题改成「文字 + 计数 + 一条延伸的横线」，替代彩色小标题
4. **最近使用改成行**：56×34 的封面缩略图 + 一行版本/来源/时间 + 开关，密度比卡片高得多，一屏能看 6 个
5. 空状态收成一张卡（图标 + 说明 + 主次按钮），不再占满整屏

### G.3 标题栏按钮的 hover 区域

`align-items: center` 让 `.titlebar-controls` 只有内容高度，按钮实际是 46×14 的一条，鼠标移上去只有图标周围变色。改成 controls 和按钮都 `height: 100%`，实测按钮 46×36；hover 底色也从 `surface-2` 换成专门的 `--s-hover`（深色 `rgba(255,255,255,.09)`、浅色 `rgba(15,20,30,.07)`），比原来的近似色明显。

### G.4 桌面快捷方式图标

排查结论：**exe 里的图标是新的**（把 exe 的 RT_ICON 抽出来核对，6 个尺寸与 `icon.ico` 逐字节一致，Shell 取到的 32×32 也是新图标）。桌面仍显示旧图标是 Windows 的图标缓存没失效。

处理方式：应用启动时如果 `settings.lastVersion != 当前版本`，调一次 `SHChangeNotify(SHCNE_ASSOCCHANGED)` 让 Shell 重建图标缓存（安装器常用手段），然后写入新版本号。这样以后每次升级都会自动刷一次，不需要用户手动清缓存。

### G.5 顺带修的一个健壮性问题

封面源探测原来是 `OnceLock`，一次探测失败就把 `"none"` 永久缓存，本次运行里再也抓不到封面（改名时实测撞到）。改成只缓存"可用"的结果；两个源都失败时不缓存，60 秒后下次请求重试。

---

## 附录 H：0.1.3（封面搜索的语言问题、布局微调）

### H.1 中文名搜不到封面的真正原因

不是匹配算法的问题，是**查了错误的语言索引**。Steam 的 `storesearch` 会根据 `l` 参数返回对应语言的名字：

| 请求 | term | 返回的名字 | 结果 |
|---|---|---|---|
| `l=english&cc=cn` | 影之刃零 | Phantom Blade Zero | 和「影之刃零」比对永远 0 分 → 判定没搜到 |
| `l=schinese&cc=cn` | 影之刃零 | 影之刃零 | 精确命中 |

之前的代码硬编码了 `l=english`（当初是为了让英文目录名匹配英文标题），所以**只有中文名会失效**。现在：

1. 用 `has_cjk()` 判断输入是否含中日韩字符
2. 中文走 `schinese` 优先，英文走 `english` 优先，各试 `cc=cn` 和 `cc=us` 两套索引（实测 `cc=cn` 有时返回空，`cc=us` 更稳）
3. 名字对不上时用 `first_usable()` 取第一条非噪音结果兜底——Steam 按相关度排序，用户手输的名字通常就在第一条
4. 网络连续两次失败就早退，把「连不上 Steam」原样报出来，而不是假装「没搜到」

`cargo test` 里加了 5 个用例覆盖：CJK 判定、中文精确匹配、英文标题不会误配中文名、噪音条目过滤、归一化。

### H.2 本地图片兜底

有些游戏本来就没有商店页（独占、未发售、独立小游戏），任何搜索接口都拿不到图。所以右键菜单加了「选择本地图片作封面」：系统文件对话框选图 → 原样存进 `covers/<id>.jpg`。

顺带修了 `data_uri` 的 MIME：原来写死 `image/jpeg`，用户放一张 PNG 进去浏览器会拒绝渲染。现在按文件头判断 PNG / JPEG / WebP / GIF。

### H.3 布局调整

- 总览头部的状态带改成独占一行、拉满内容宽度（原来被右侧按钮挤在半栏里）
- 顶部的「扫描 / 添加」两个按钮移进右栏快捷操作区，头部只留信息
- 页脚的非官方声明改为居中
- 侧栏左下角的状态卡整个去掉，只留「设置」——总览页已经有同样的信息，重复了
- 标题栏的应用名真正居中：原来「ANR Studio」和版本号是一个 flex 行整体居中，导致名字本身偏左 21px。现在版本号绝对定位挂在名字右侧，不参与居中计算（实测名字中心 = 窗口中心 640）

### H.4 应用原来不走系统代理

调试时发现一个更底层的问题：这台机器上 `curl` 能打开商店页，应用却报「连不上 Steam」。

原因是 **reqwest 默认既不读 `HTTP_PROXY` 环境变量，也不读 Windows 的 Internet 设置**，而国内用户几乎都靠系统代理（Clash / v2rayN 之类）上网。浏览器和 curl 走代理，应用走直连，于是同一个地址一个通一个不通。

现在所有网络请求统一走 `net.rs`：

- 先看环境变量（`HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY`，大小写都认）
- 再读注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` 的 `ProxyEnable` / `ProxyServer`（兼容 `127.0.0.1:7890` 和 `http=...;https=...` 两种写法）
- 覆盖三处：封面搜索、GitHub release 列表、安装包下载

这也解释了之前「封面时好时坏」的现象：不是算法问题，是网络路径不稳定。

### H.5 怎么在没有网络的情况下验证

Steam 在调试环境里不可达，所以加了一段只在 debug 构建生效的地址覆盖：

```
ANR_STEAM_BASE=http://127.0.0.1:8899   # 商店搜索接口
ANR_STEAM_CDN=http://127.0.0.1:8899    # 封面图 CDN
```

配 `scripts/mock_steam.py`（本地假商店：只有 `cc=us&l=schinese` 返回「影之刃零」，其余返回空）跑一遍，服务端日志确认了完整顺序：

```
GET /api/storesearch/?term=影之刃零&cc=cn&l=schinese   -> 200 空（跳过）
GET /api/storesearch/?term=影之刃零&cc=us&l=schinese   -> 200 两条（精确命中「影之刃零」）
GET /steam/apps/4405120/header.jpg                     -> 200
```

最终 `refresh_cover` 返回 `data:image/png;base64,...`，41ms。也就是说「语言分流 → 分区回落 → 名字匹配 → 下载 → 落盘」这段逻辑是通的，剩下唯一变量是用户网络能不能到 Steam。

---

## 附录 I：0.1.4（图标、封面国内源、空态介绍）

### I.1 封面来源：为什么选 360 图片

需求是「国内也能搜到封面」。把候选源挨个试了一遍：

| 站点 / 接口 | 结果 |
|---|---|
| TapTap `webapiv2/app-search/*` | 全部 404，网页是 Vue SPA，JS 资源对非浏览器请求返回同样的占位页 —— 拿不到 |
| 百度图片 `acjson` | 返回 `antiFlag` 反爬载荷，没有结果 |
| 百度百科 / 游民星空 / 3DM 搜索页 | 403 或纯 JS 渲染 |
| 搜狗图片 `napi/pc/searchList` | `{"status":1,"info":"forbid"}` |
| **360 图片 `image.so.com/j`** | ✅ 200，返回 JSON：标题、原图/缩略图地址、宽高 |

所以国内源用 360 图片。图片搜索最大的坑不是搜不到，而是**配错游戏**——搜「影之刃零」的第一版实现返回了《影之刃3》的宣传图。现在的策略：

1. 查询词只用游戏名本身（加「游戏封面」反而会引入大量无关新闻图，实测如此）
2. **硬性过滤**：结果标题里必须出现游戏名（归一化后包含，或前 4 个字符命中），否则整批丢弃，宁可报「没匹配到」也不给错图
3. 排序：横版（1.35–2.1）加分、宽度 ≥1000 加分；标题含「游戏/封面/海报/预告/公布/官宣」加分；含「攻略/解说/售价/预约/脸模/试玩/直播」减分
4. 依次尝试前 5 个候选下载（部分站点会 403），校验字节数与图片魔数

实测「影之刃零」拿到 1080×608 的官方风格横版主视觉，「艾尔登法环」拿到 940×528 的官方 key art。

### I.2 来源设置

`settings.coverSource`：`auto` / `domestic` / `global`。

- `global`：Steam → GOG
- `domestic`：只用 360 图片
- `auto`：先用 `provider()` 的探测结果，国外服务不可达时直接走国内（避免让用户干等 12 秒超时），可达时顺序为 Steam → GOG → 国内

### I.3 更新日志「全都显示没有」

原因是 `get_changelog` 依赖 `get_releases()`，而后者只在内存缓存 10 分钟内有效。缓存过期后如果 GitHub 恰好连不上，命令直接报错，前端把它吞成了「没有提供日志」——网络一抖，所有版本看起来都没日志。

修法两层：

1. 后端：有内存缓存就用（哪怕已过期），只有一次都没拉到过才请求网络
2. 前端：区分「没有正文」和「读取失败」，失败时显示具体原因并提供「重试」按钮

### I.4 布局

- 总览容器改成纵向 flex + `min-h-full`，页脚用 `mt-auto` 真正贴底（原来只是一段 `mt-8` 的普通文字，看着悬在半空）
- 空库时左栏补一块「关于 ANR 与这个工具」：ANR 是什么 / 这个工具做什么 / 需要注意（反作弊风险），底部一排快捷入口
- 图标换成手柄版 ANR STUDIO（1920×1920 圆角白底），全套 16–512 与 ico 由 `tauri icon` 重新生成

---

## 附录 J：0.1.5（卡片右键菜单、改目录、移除）

### J.1 「关闭帧生成」和「复原」不是一回事

这点需要在界面上讲清楚，因为很容易误解：

| 操作 | 做了什么 | 结果 |
|---|---|---|
| 帧生成开关 → 关 | 只把 `OptiScaler.ini` 的 `[FrameGen] Enabled` 写成 `false` | 文件、代理 DLL、provider 全都在，游戏下次启动就不再生成帧 |
| 卸载 | 跑官方 `uninstall.ps1`：按 `nr_fg_install.json` 里的哈希校验后还原原文件，删掉 `OptiScaler` 目录 | 目录回到安装前的样子 |
| 卸载并删除 | 卸载 + 从列表移除卡片 | 上一条 + 列表里不再出现 |

所以「关掉开关」只是停用，不占用磁盘的是卸载。抽屉里那行说明已经改成「关闭只是停用，文件仍留在游戏目录」。

### J.2 右键菜单重新分组

按「先对目录动手 → 再改显示 → 最后看详情」的顺序排，用两条横线分成三组：

```
更改游戏目录
打开游戏目录
复制目录路径
卸载并删除 / 从列表移除      ← 红色，随安装状态换文案
────────────
修改游戏名
重新搜索封面
删除封面                      ← 没有封面时置灰
选择本地图片作封面
────────────
查看详情
```

`ContextMenu` 的 `MenuAction` 新增了 `disabled` 字段，用于「删除封面」这种无意义的场景。

### J.3 更改游戏目录

新增 `change_game_dir(id, path, migrate)`：

1. 校验新目录存在、且里面有 `.exe`（避免选到空的父目录）
2. `migrate = true` 时先在旧目录跑卸载（含快照），再更新设置
3. 旧目录若是手动添加的 → 直接改路径；若是扫描出来的 → 旧目录进 `hiddenGames`，新目录加入手动列表
4. `rescan()` 后返回新列表；目录变了 id 也会变，前端据此清掉旧抽屉

前端：点「更改游戏目录」→ 选文件夹 → **如果旧目录装着 ANR**，弹 `MoveDirDialog` 让用户选「卸载并迁移 / 只改目录 / 取消」；没装就直接改。

### J.4 移除卡片

`remove_game(id)`：手动添加的从 `manual_games` 删掉；扫描出来的记进 `hidden_games`，`rescan` 时过滤，重启也不会再冒出来。文件一个都不动。

### J.5 删除封面

`clear_cover(id)`：删掉 `covers/<id>.jpg` 并 emit 事件，卡片立刻回到字母占位图。菜单项在没有封面时置灰。

---

## 附录 K：0.1.6（前置检查 / 取消任务 / 失败回滚）

这是「不会坏」这条承诺欠下的三笔账。

### K.1 安装前置检查（precheck.rs）

文档 5.2 写的是「全部通过才继续」，之前只有目录和进程两项在 `do_install` 里硬拦。现在独立成 `precheck_install(id)`，前端在**开始安装之前**先问一次：

| 检查 | 级别 | 依据 |
|---|---|---|
| 目录存在 | block | `is_dir()` |
| 目录里有 .exe | block | `scan::find_exe` |
| 游戏没在运行 | block | `tasklist` 比对 exe 名 |
| 目录可写 | block | 写一个临时文件再删 |
| 剩余空间 ≥ 300MB | block | `GetDiskFreeSpaceExW`（裸 FFI，不引依赖） |
| VC++ 2015-2022 x64 | warn | System32 下四个 DLL，缺了游戏能开但 ANR 不生效 |
| 疑似加壳 | warn | `artifact.dll` / `coldloader.dll` / `vmprotect` 等特征文件 |
| 反作弊组件 | warn | EasyAntiCheat / BattlEye / ACE / SGuard 等目录名，受设置的「反作弊风险提示」开关控制 |

block 直接拒绝（toast），warn 弹确认框（琥珀色，列出每条原因），用户点「仍然安装」才继续。`ConfirmRequest` 为此加了 `onCancel`，`tone` 支持 `warn`。

**加壳检测是这一版真被打脸换来的**：用户那个 511MB 的虚拟机破解版，装完 dxgi.dll 后游戏直接打不开，卸载才恢复。安装记录里 `originals` 是空的（没覆盖任何原文件），所以问题就是壳不接受多出来的模块。现在装之前就会提醒。

### K.2 取消任务真的能取消

之前前端的「取消」只是把界面状态改成已取消，后端照跑不误。现在：

- 全局 `CANCELED: HashSet<taskId>`，`cancel_task(task_id)` 写入
- `download_to` 每读 128KB 检查一次，命中就删掉半包并返回「已取消」
- `ensure_package` 在开始前、下载中、校验前都检查
- `do_install` 在拿到包之后、执行脚本之前各查一次
- `emit()` 在 done / error / canceled 时把标记清掉，避免堆积

### K.3 失败回滚

- 切换版本 / 覆盖安装之前，先把当前 `OptiScaler` 目录打成快照
- 脚本或校验失败时，`restore_snapshot` 把快照解回去（先删掉半成品目录再解压），并把错误文案追加「（已回滚到切换前的状态）」
- 首次安装失败没有旧状态可回滚，就只报错，让用户手动卸载

### K.4 顺带补的小项

- 托盘 tooltip 变成「ANR Studio · 已安装 N · 已启用 M」
- 卡片状态旁边加「运行中」标签（之前只在抽屉里禁用按钮，列表上看不出来）
- 日志页加「复制并开 Issue」：复制日志 + 打开 GitHub Issue 新建页
- 版本页的「N 个游戏」其实早就在了（之前核对时漏看），不用改

### K.5 修正一处之前的误判

上一轮我写的差距分析里说「Epic / GOG 扫描未实现」，这是错的：`scan.rs` 里 `epic_games()` / `gog_games()` 都在，只是我 `grep` 时被 steam 相关的十几行刷掉了，没往下翻。实际只支持 Steam 的说法不成立。

---

## 附录 L：0.1.7（拖拽添加 / 首次引导 / 原生 DLSS-G 开关）

### L.1 拖拽文件夹添加

`App.tsx` 里用 `getCurrentWebview().onDragDropEvent` 监听 `enter / over / drop / leave`：

- 拖到窗口内 → 全屏半透明遮罩 + 虚线框「松开以添加游戏」
- 松开 → 逐个调用 `add_manual_game`，按数量给 toast；一个都没加成功时提示「文件夹里没有 .exe，或已经添加过了」
- 监听器在组件卸载时注销，避免热更新后叠好几个

### L.2 首次启动引导

`settings.onboarded` 为 false 时盖一层引导，三步讲清「添加游戏 → 装 ANR → 游戏里开帧生成」，底部显示已经找到几个游戏，两个出口：「添加游戏目录」「开始使用」。点开始使用写回 `onboarded: true`，之后不再出现。

这一步同时承担了「空库时用户不知道干什么」的问题——之前总览的空状态只有一句说明。

### L.3 帧生成实现可以在 ANR 和原生 DLSS-G 之间切

文档 Q1 的问题是「`FGNvngxReplacement=auto` 是否等价于旁路到原生 DLSS-G」。没实测之前不敢猜，所以做成一个显式开关，让用户自己 A/B：

抽屉里新增「帧生成实现」，两个按钮写 `OptiScaler.ini` 的 `[FrameGen] FGNvngxReplacement`：

| 选项 | 写入值 | 含义 |
|---|---|---|
| ANR | `Apex Neural Rendering FG` | 用 ANR 生成中间帧，任何显卡都能用 |
| 原生 DLSS-G | `None` | 旁路到 NVIDIA 官方实现，只有 40 系生效 |

`AnrState` 新增 `fgReplacement` 字段，界面据此高亮当前选项；写之前会 `backup_once`，改完提示「重启游戏生效」。

### L.4 还没做的

- 批量操作条只覆盖安装/开关，没有批量卸载
- 键盘 Tab 顺序没有专门调过（焦点环有）
- 多语言、指纹表远程更新（Q2/Q6）

---

## 附录 M：0.1.8（画面调校：中文调 OptiScaler 参数）

### M.1 为什么不做"实时"，也不替换游戏内菜单

用户问过能不能把游戏里的英文菜单换成我们的。技术上要自己注入 DLL + hook D3D12 Present + 画一套 ImGui，还要和 OptiScaler 自己的覆盖层抢 hook —— 投入比整个 ANR Studio 还大，且直接踏进反作弊雷区。

改成读 `OptiScaler.ini`：不需要注入，中文标签、分组、说明、危险项校验都能做。唯一代价是**改完要重启游戏**，因为 OptiScaler 只在启动时读 ini。

反向也成立：游戏里按 Save Settings 会写 ini，界面上点刷新就能看到当前值——半个实时，方向是游戏 → App。

### M.2 一条必须遵守的约束

OptiScaler 退出时会把自己的内存值写回 ini，覆盖我们在游戏运行期间的修改。所以：

- `read_opti_tuning` 返回 `running`，界面顶部显示琥珀色提示并禁用所有控件
- `write_opti_tuning` 在 `game.running` 时直接返回错误，不给"看起来改了其实会被覆盖"的错觉

这不是体验优化，是正确性问题。

### M.3 全量 vs 翻译

ini 里实测有 **38 个分节 / 362 个键**。全量上界面，但中文目录只覆盖常用的一批：

- 每个键仍然展示，未翻译的用原始键名 + 「原键名」小标签，默认折叠，按分节展开
- 整节都没翻译的（FSR3 生成、XeSS 生成这类），收成一行「展开 N 项原始键名」，不占一整张空卡片
- 控件类型从值推断：`true/false/auto` → 三段按钮，数字 → 数字输入，其余 → 文本；有中文目录的按目录给枚举选项

### M.4 写入安全

- 每次写之前 `IniDoc::backup_once`（同目录 `.anr-bak`）
- 按目录校验取值：bool3 只收 `true/false/auto`，number 必须能解析，enum 必须在选项内
- 值不允许换行，长度上限 200
- 用 `IniDoc` 而不是重新生成文件，注释和顺序都会保留

### M.5 实测

- 解析 2077 的 ini：`fields: 362`，与键数一致
- 写 `Sharpness/Sharpness = 0.55`：文件里确实变成 `Sharpness=0.55`，回读一致
- 写非法值 `FrameGen/Enabled = maybe`：被拒，「这个开关只能是 true / false / auto」
- 游戏运行中写 2077 的 ini：被拒，提示 OptiScaler 退出会覆盖

---

## 附录 N：0.1.9（调校页对齐游戏内菜单 + 中英双语）

### N.1 分类照抄菜单

原来自己分了「上采样 / 帧生成 / 兼容 / 菜单日志」四组，和游戏里对不上号。现在 TABS 与 SECTIONS 两张表直接按菜单排：

| Tab | 中文 | 包含的 ini 分节（按顺序） |
|---|---|---|
| upscaling | 上采样 Upscaling | Upscalers → Sharpness → CAS → OutputScaling → UpscaleRatio → QualityOverrides → DRS → Inputs → InitFlags → FSR/DLSS/DLSSD/XeSS → Reprojection |
| framegen | 帧生成 Frame Gen | FrameGen → Framerate → OptiFG → DLSSG → NvngxFG → FSRFG → FSRFGInputs → XeFG → V-Sync |
| latency | 延迟 Latency | fakenvapi → NvApi |
| textures | 纹理 Textures | Anisotropy → Mipmap → HDR |
| compat | 兼容 Compatibility | Hooks → Libraries → Plugins → Spoofing → Hotfix → Dx11withDx12 → ProcessFilter |
| menu | 菜单 Menu | Menu → Log |
| quirks | 其它 Quirks | Magnifier |

Tab 顺序、中文名、英文名都由后端 `Snapshot.tabs` 下发，前端不再自己维护一份，避免两边漂移。

### N.2 字段顺序

`META` 表里的书写顺序就是同分节内的展示顺序（先前的表是按主题写的，这次按菜单重排）。没有中文目录的键排在已知项之后，保持 ini 里的原始顺序。

排序键是四元组 `(tab 序, 分节序, 字段序, 文件序)`，稳定且可预测。

### N.3 中英双语

每行显示 `中文名 English Name`，英文用更小的字重和灰色；分节标题同样 `超分器 Upscalers`；Tab 是两行（中文在上、英文 + 数量在下）。布尔开关也写成 `开 On / 关 Off / 自动 Auto`。

未翻译的键显示原始键名 + 「未翻译」小标签，仍然可以改。

### N.4 一处修正

之前 `Anisotropy` / `Mipmap` 同时挂在「上采样」和「纹理」两处（写的时候偷懒用了 `TexAnisotropy` 这种假节名），会导致纹理页整页空掉。现在只保留在 Textures 下，且用真实 ini 节名。

---

## 附录 O：0.2.0（调校页按原始面板重建 + 控件升级）

### O.1 从「ini 分节」改成「菜单模型」

上一版按 ini 分节分组，和游戏里看到的顺序仍然对不上。用户提供了 9 张原始面板截图（Custom / Upscaling / Frame Gen / Latency / Textures / Compatibility / Menu / Quirks / Status），这次直接以菜单为准：

| Tab | 分组（按顺序） |
|---|---|
| 上采样 Upscaling | Upscalers → Sharpness → Upscale Ratio Override → Output Scaling → Magnifier |
| 帧生成 Frame Gen | Frame Generation → Framerate |
| 延迟 Latency | Low Latency → V-Sync Settings |
| 纹理 Textures | Mipmap Bias → Anisotropic Filtering → HDR |
| 兼容 Compatibility | Upscaler Inputs → Startup → Shaders → Hooks → Plugins → Spoofing → Hotfix |
| 菜单 Menu | Menu → FPS Overlay → Keybinds → Logging |
| 状态 Status | System（只读，显示目录 / 代理 DLL / 安装时间 / provider / 日志是否生成） |

`ITEMS` 表的书写顺序就是界面顺序；ini 里没被菜单收录的键归到「<分节> · 其它」折叠组。Custom / Quirks 是菜单自身的状态页，跳过。

### O.2 控件类型

每一项现在带 `kind`，不再是清一色的文本框：

| 控件 | 用途 | 例子 |
|---|---|---|
| toggle（三态单选） | 开 / 关 / 自动 | 覆盖锐化、启用输出缩放 |
| select（下拉） | 有固定候选 | 超分器、FG Input、FG Nvngx、日志级别 |
| slider（滑块 + 数值 + auto 按钮） | 有合理区间 | 锐化强度 0–1/0.05、生成帧数 0–5/1、DMFG 目标 30–240/5、Mipmap 偏移 −2–2/0.01 |
| number / text | 自由输入 | 帧率上限、伪装显卡名、快捷键 |
| 多选矩阵 | Upscaler Inputs 表 | FSR2 / FSR3 / FFX / DLSS × 启用 / 使用 / 匹配串，点方格在 开-关-自动 间轮换 |
| status | 只读 | 状态页 |

值写成 `auto` 时滑块显示为「自动」，点右边的 `auto` 按钮可随时回到跟随默认。

### O.3 一个细节

`FGInput` 的真实取值是 `NvngxFG`（首字母大写），之前下拉里写的是小写 `nvngxfg`，会把当前值显示成空。现在下拉候选使用真实拼写，且读到的值不在候选里时自动补进去，避免显示空白；写入校验也同步放宽（`auto` 始终允许）。

### O.4 验证

- 解析 2077 的 ini：7 个 Tab / 367 项，控件类型正确（`Sharpness → slider`、`FGInput → select`）
- 写 `Sharpness/Sharpness = 0.55` 落盘成功；非法开关值被拦
- 各 Tab 的分组顺序逐页核对，与截图一致

---

## 附录 P：0.2.1（补齐差距分析里的核心缺口）

对照附录 L.4 与上一轮的差距分析，把「核心功能缺口」逐项落地。

### P.1 下载健壮性（R5 / R6 / 5.2）

- `installer.rs::download_to` 改为写 `<zip>.part` 并带 `Range: bytes=N-` 续传：206 追加、200 从头写、416 清残留。取消或下载不完整都**保留 .part**，下次自动从断点继续；下完才改名为正式 zip。上次已下完但未改名的残留直接复用（仍会走 SHA256 校验）。
- `ensure_package` 不再吞掉校验和：先取官方 `SHA256SUMS.txt`，取不到 / 找不到对应条目 / 哈希不匹配一律拒绝安装，错误信息给出「切换下载源或手动导入本地包」的出路，**不允许跳过**（R6）。zip 已存在时先校验再决定是否重下。
- 新增 `import_local_package` / `pick_zip_file` / `official_sum`：GitHub 拉不动时导入官方 zip，SHA256 可从官方 sums 自动填或手动粘贴，校验不过直接拒绝（R5）；导入成功后从包内 `nr_fg_package.json` 补录指纹。

### P.2 安装队列串行（6.5.4）

- 新增 `jobs.rs`：`Mutex<usize>` + `Condvar` 的计数闸门，`Ticket` 用 RAII 归还名额。`concurrency` 不再是空壳，设置页可选 1 / 2 / 3，默认 1 = 严格串行。
- `install` / `uninstall` / `switch_version` / `prefetch` 都先过 `task_gate`。排队时任务卡显示「排队中」，排队中取消仍生效；任务提前返回也不会漏还名额。

### P.3 版本管理收尾（6.4④ / Q2 / R7）

- `Settings.defaultVersion`（空 = 跟随最新）：`set_default_version` 落盘，版本页每行「设为默认 / 取消默认」+「默认」标记，设置 → 关于显示当前值。`install` 未指定 tag 时优先用它。
- 新增 `catalog.rs` + `refresh_catalog`：重扫 `cache/<tag>/<kind>/nr_fg_package.json` 补录指纹，并尝试拉远程 `catalog.json`（拉不到不算失败，仍完成本地刷新），返回新增条数与总数（Q2）。
- `mark_version(id, tag)`：把该游戏 provider DLL 的 SHA256 写进指纹表并重新扫描，解开「未知版本」死结（R7）；版本页「手动标记」弹窗选择游戏 + 版本。

### P.4 状态矩阵与自检（5.1 / R8 / R9）

- `inspect::detect_conflict` 实现 5.1 里「需人工确认」的三类异常：未知来源残留、ini 丢失、状态不完整。`Game.conflict` 下发前端：卡片状态变「需确认」、进「需注意」筛选、抽屉显示原因条；安装/卸载前弹确认，后端要求 `force=true` 兜底并放行 `do_install` 的第三方 OptiScaler 检查。
- `check_package_structure`：启动时检查缓存里所有 `.ok` 包是否还有 `install.ps1`，失效则提示「官方包结构已变更，请更新本工具」（R8）。
- `precheck::steamapps_warning`：安装前置检查给出 Steam 库 warn，`add_manual_game` 返回同一提示由前端 toast（R9）。

### P.5 首启引导（6.5.8）

`Onboarding` 列出最多 4 个可安装游戏 + 剩余计数，新增「一键全部安装（N）」，配合队列串行执行；保留「添加游戏目录 / 开始使用」两个出口。

### P.6 验证

- `npx tsc --noEmit` 通过；`cargo test --lib` **17/17**（新增 jobs 3 个、catalog 2 个、installer 2 个，连同上一轮 releases 5 个）；`npm run build` 通过。
- headless Chrome 实渲染核对：版本页新增按钮与默认版本说明、导入包弹窗、手动标记弹窗、设置页「同时进行的任务数」与「校验与指纹」、引导页一键全装列表，均无错位。
- 重新 `npx tauri build` 生成 0.2.0 安装包（版本号未升，需重启应用生效）。

### P.7 还没做的（缺口清单里剩下的）

- 体验类：界面多语言 i18n（Q6）、网格/列表选择持久化（6.4②）、Tab 顺序专门调整（6.5.12）。
- 部分实现：Conflict 已有独立状态，但人工确认门禁只加在安装/卸载两个入口；Q1 的 auto 三态仍是 ANR / 原生 DLSS-G 二态（L.3 设计变更）；版本「查看说明」仍用更新日志面板 + GitHub 原页代替 notes_url。
- 范围外不变：nr-record / SR/RR / Linux Proton / 静默自动更新 / 注入覆盖层。

---

## 附录 Q：0.2.2（体验类补齐：多语言 / 布局持久化 / 键盘可达）

### Q.1 多语言（Q6）

按用户指定的位置与交互实现：

- 切换入口在 **TopBar 右上角、深浅色主题图标之前**，按钮显示当前语言的自绘 SVG 小国旗（Windows 的 emoji 字体不渲染国旗区域指示符，所以用 SVG 而不是 emoji）。默认简体中文，点开菜单可在「简体中文 / English」之间切换，当前项打勾。
- `Settings.locale` 落到 settings.json，启动时恢复；`zh` 是源码原样，因此中文界面零开销。
- 实现采用**字典式运行时覆盖层**（`src/i18n/`）：组件不改，切换语言时 App 用 `key={locale}` 重新挂载整棵子树，再对这棵新树按字典替换文本节点与 `title` / `aria-label` / `placeholder` 属性；`MutationObserver` 负责后续 React 动态更新。切回中文靠重挂载自然恢复，不需要反向字典。
- **只做精确匹配与整句规则，不做子串替换**：早期试过子串替换，把「写入版本资源」翻成了「写入Versions资源」，长句里的「版本 / 工具 / 游戏」被短词乱塞。现在未收录的片段宁可保留中文，也不产出坏句子。
- 字典键就是**渲染出来的文本片段**。`scripts/` 下留了一整套工具链：
  - `i18n-scan.mjs` 从源码抽中文 → `_i18n_raw.json`
  - `i18n-collect.mjs` 用 CDP 遍历各页面/弹窗，收集真实渲染文本（含内联标签拆开的片段、动态数字句）→ `_collected.json` / `_missing.json`
  - `i18n-en.txt` / `i18n-extra.txt` 是译文表（按清单顺序，一行一条）
  - `i18n-build.mjs` 编译成 `src/i18n/en.json`，行数不符会直接报错
  - `i18n-verify.mjs` 在英文界面遍历各页面，列出残留中文
  - `index.ts` 里的 `RULES` 覆盖带变量的整句（如「已选 N 个」「全部更新到 X」「可回滚到 X」），`PATCH` 用于少量手工补漏
- 目前字典 522 条 + PATCH 若干。**新增一种语言**只需：加一份 `xx.json` 字典、在 `LOCALES` 注册、给 `Flag` 加一个分支，界面其余部分不用改。

### Q.2 记住网格 / 列表选择（6.4②）

`Settings.layout` 落盘，`setLayout` 同时写回设置，`init` 启动时恢复。之前只在内存里，重启必回网格视图。

### Q.3 键盘可达（6.5.12）

`src/lib/focus.ts` 提供全局焦点管理：弹窗出现时自动聚焦第一个输入/按钮，Tab 与 Shift+Tab 在最上层 `.anim-panel` 内循环，不会跑到弹窗后面的页面。焦点环沿用已有 CSS，不改各弹窗组件。

### Q.4 顺带补齐的「部分实现」

- **Conflict 门禁覆盖全部写操作**：原来只有安装/卸载弹确认，现在切换版本、帧生成开关、切换帧生成实现也会先确认；后端抽了 `require_confirmation()`，四类命令统一校验 `force`，不只是前端拦一下。
- **Q1 三态开关**：帧生成实现从 ANR / 原生二态补成 **ANR / 原生 DLSS-G / 自动(auto)** 三态；`set_fg_replacement` 支持 `auto`，抽屉按 `FGNvngxReplacement` 的实际值高亮。

### Q.5 验证

- `npx tsc --noEmit` 通过；`cargo test --lib` **18/18**；`npm run build` 与 `npx tauri build` 成功。
- headless Chrome 实测：中文 → English → 中文往返正常；遍历总览 / 游戏库 / 版本 / 调校 / 日志、设置五个分类、导入包 / 手动标记弹窗、游戏详情抽屉、命令面板，**英文界面残留中文 0 条**。
- 截图核对：国旗按钮在主题图标左侧、菜单两项带国旗与勾选、抽屉三态按钮、英文设置页并发数项，布局均无错位。

### Q.6 还没做的

- 界面多语言目前是「简体中文 / English」两种（架构可扩展，字典与工具链已备好）；如需日语等，按 Q.1 的步骤加字典即可。
- 版本「查看说明」仍用更新日志面板 + GitHub 原页代替 `notes_url` 字段。
- 范围外不变：nr-record / SR/RR / Linux Proton / 静默自动更新 / 注入覆盖层。

---

## 附录 R：0.2.3（快照可恢复 / 安装失败闭环）

这一版补的是「不会坏」承诺里欠得最重的两笔：快照能真正恢复，失败后用户有下一步可点。

### R.1 快照从「只读备份」变成「可恢复状态」

旧快照只打包 `OptiScaler/`，卸载后即使手工解压，也会缺 `dxgi.dll` 和 `OptiScaler.ini`。现在快照升级为 schema 2：

- zip 内包含 `OptiScaler/`、根目录代理 DLL（从 `nr_fg_install.json` 的 `proxy` 读取）、`OptiScaler.ini`
- 额外写 `snapshot.json`：`gameId` / `gameDir` / `gameName` / `proxy` / `createdAt`
- 恢复时只允许写 `OptiScaler/`、`OptiScaler.ini`、`dxgi.dll` / `winmm.dll` / `version.dll`，防止被改过的 zip 往游戏目录塞任意文件
- 旧快照（schema 0）仍可恢复，但只恢复 `OptiScaler/`，界面会标「旧版」并提示可能缺代理 DLL / ini
- 抽屉 →「快照」可以列出当前游戏的快照并恢复；新快照按 `gameId` 匹配，游戏改名后仍能找回
- 恢复前检查游戏未运行；恢复后重新扫描卡片状态

### R.2 安装 / 切换失败后的三个出口

- `run_install` 失败时把 stdout/stderr 原始输出一起带回，不再只留最后一行
- 失败弹窗提供：**重试 / 生成诊断包 / 回滚到快照**，并在可滚动区域展示原始输出
- 后端在覆盖安装失败时仍会自动回滚；错误里用 `[[ANR_SNAPSHOT]]` / `[[ANR_ROLLED_BACK]]` 标记把快照路径和回滚结果传给前端
- 自动回滚成功时弹窗明确写「已自动回滚到操作前的状态」；自动回滚失败时「回滚到快照」按钮可用
- 首次安装没有旧状态，回滚按钮置灰并说明「没有可用的安装前快照」

### R.3 验证

- `cargo test --lib`：20/20，新增「快照包含代理 DLL 与 ini 并能完整恢复」「旧快照跨游戏拒绝恢复」两个用例
- `npx tsc --noEmit` 通过；`vite build` 通过；NSIS 安装包构建通过

---

## 附录 S：0.3.0（任务中心 / 批量操作 / 体检 / 实时运行状态）

这一版补的是「承诺了但没做到」的四件事，外加两处会坏数据的细节。

### S.1 批量操作之前是坏的

`批量安装 / 批量卸载` 用 `selected.forEach(id => install(id))` 并发触发，而确认框只有一个槽位：
后一个 `askConfirm` 会把前一个顶掉，前面的 Promise 永远不 resolve。结果是「点了批量，只有最后一个真的执行」。

现在的做法：

| 项 | 做法 |
|---|---|
| 确认 | `askConfirm` 带队列（`confirmQueue`），依次弹出；批量流程只弹**一次**汇总确认 |
| 执行 | 顺序 `await`，不并发；后端的任务闸门仍然保证真正的串行 |
| 结果 | 每个失败项记 `{游戏名, 原因}`，最后给一条「成功 N、失败 M」的 toast，前两条原因展开写 |
| 跳过项 | 运行中 / 安装状态需确认的游戏直接跳过并计入失败原因，不做「假成功」 |
| 新增 | 批量**更新到最新**（`switchMany`），替换原来只能一个个点 |

### S.2 任务中心

原来只有右下角 3.2 秒自动消失的浮卡，取消的任务卡还会永久留在界面上且关不掉。现在：

- 任务不再自动删除，进入 `tasks` 列表（上限 40，超出丢最老的已结束任务）；取消态也能移除
- 新增右侧「任务中心」（顶栏按钮，`Ctrl+Shift+J`）：历史、进度、失败原因（后端把错误放进 `stage`，前端收进 `detail`）、重试、取消、清除记录
- 浮卡只保留「进行中 + 最近一条结果」，其余折叠成「还有 N 条任务」入口
- 后端 `emit` 在 `canceled` 状态也会清掉取消标记，避免误取消后续同 id 任务

### S.3 运行状态实时化 + 所有写操作实时复查

之前只有 `install` 会重新查进程，`uninstall` / 开关 / 调参 / 快照恢复都只看上次扫描的 `game.running`。
应用开着的时候启动游戏，界面还显示「未运行」，于是：卸载会删一半就失败，改 ini 会被 OptiScaler 退出时覆盖。

| 层 | 改动 |
|---|---|
| 判定 | `scan::game_running`：进程名匹配 **或** 代理 DLL 被独占（`ERROR_SHARING_VIOLATION` 32 / 33）。只看共享冲突，避免只读目录被误判成「游戏中」 |
| 后端 | 安装、卸载（含排队后再查一次）、切换版本、帧生成开关、`write_config`、`write_opti_tuning`、`restore_snapshot`、修复，全部实时复查 |
| 前端 | `get_running_ids` 每 5 秒轮询（页面不可见时跳过），只更新真正变化的 `running` 标记 |

### S.4 安装体检与一键修复

`health_check` 返回问题清单，`repair_install` 返回 `{fixed, remaining, needsReinstall}`：

- 能本地修的：`OptiScaler.ini` 丢失但有 `.anr-bak` → 还原；`FGNvngxReplacement` 不是 ANR → 写回；缺 `Enabled` → 补 `auto`；代理 DLL 丢失且备份哈希校验通过 → 还原
- 修不了的：provider 目录/`nvngx_dll` 丢失、没有备份，明确返回「需要重新安装」，不假装修好
- 抽屉里新增「安装体检」区块，可重新检查；修复前同样实时查进程

### S.5 安装脚本不会再挂死

`cmd.output()` + 隐藏窗口 + 继承 stdin：脚本一旦走 `Read-Host`（缺参、异常分支）就永久挂住，界面只有转圈。
现在：`-NonInteractive` + `stdin(Stdio::null())` + stdout/stderr 独立线程收集 + 120ms 轮询 `try_wait` + 取消与 10 分钟超时都会 `kill` 子进程；错误结构里带 `canceled`，任务卡显示「已取消」而不是「失败」。
解压另外加了 512 MB 上限（防解压炸弹），快照从「入队时拍」改成「拿到任务名额后拍」，避免拍到别人写到一半的目录。

### S.6 ini 写入的三处修复

| 问题 | 之前 | 现在 |
|---|---|---|
| 行内注释 | 整行重写成 `key=value`，`; 说明` 被吃掉 | 只替换 `=` 右边的值，缩进 / 键名 / 行尾注释原样保留 |
| 重复节 | 目标是文件最后一节且键不存在时，会追加第二个 `[Section]` | 用「节头 → 下一个节头」的区间定位，插到节内最后一个非空行之后 |
| 编码 | `from_utf8_lossy` 解码后一律写 UTF-8，GBK 注释变问号且不可逆 | 检测到非法 UTF-8 直接拒绝写入，并提示用 UTF-8 另存 |

### S.7 主题「跟随系统」

`theme === 'light' ? 'light' : 'dark'` 让 `system` 恒等于深色。现在 `resolveTheme` 解析系统偏好，并监听 `prefers-color-scheme`，切换系统主题即时生效。

### S.8 性能：重命令离开主线程 + 扫描缓存

非 async 的 Tauri 命令在主线程执行 —— 之前 `scan_library`、开关、调参、改名（含网络抓封面）、`list_releases`（GitHub API）都在主线程，
拨一个开关就会卡住整个窗口。现在这些命令全部 `#[tauri::command] async` + `spawn_blocking`（通过 `app.state::<AppState>()` 在阻塞线程里拿状态）。

另外两处缓存：

- `scan::find_exe`：`(目录, 目录 mtime)` 命中后不再递归遍历
- `inspect::sha256_file_cached`：provider DLL 的 SHA256 按 `(大小, mtime)` 缓存——30 MB 的 DLL 之前每改一次配置就重算一次

### S.9 顺带修掉的

- 取消与失败区分：`err_status`，任务卡不再把「已取消」显示成红色失败
- toast：失败常驻（手动关）、最多 6 条，避免批量安装堆 20 条把界面盖住
- `Esc` 层级：确认框 → 命令面板 → 任务中心 → 抽屉
- 启动页可选「画面调校 / 日志诊断」；侧栏那个错挂在「日志诊断」上的任务徽标移到顶栏任务中心按钮
- 诊断包里的版本号不再硬编码 `0.1.0`；UA 与 GitHub API UA 用 `CARGO_PKG_VERSION`
- 更新日志翻译改用 `net.rs` 的统一客户端（原来自己建 client，等于绕过系统代理）
- `tuning::write` 校验节名/键名字符集，堵住「key 带换行 = 往 ini 注入行」

### S.10 验证

- `cargo test --lib`：**37 passed / 0 failed / 1 ignored**（新增 ini 6 个、tuning 2 个、health 3 个）
- `cargo check --all-targets` 无警告
- `node tsc --noEmit` 通过；`vite build` 通过（单文件 521 KB）
- 说明：测试仍写 `std::env::temp_dir()`，在禁止写临时目录的受限环境里会失败（与代码无关，后续换 `tempfile`）

### S.11 还没做的

- 代码签名 / 自动更新 / 崩溃捕获（v0.4）
- 日志实时跟随与帧数统计（v0.4）
- 配置导入导出、预设档、收藏/标签/排序（v1.0）
- 仍不支持 Linux/Proton、nr-record、SR/RR
- 本版新增文案先手工补进了 `src/i18n/en.json`；下次按工具链重跑 `i18n-collect` 时，记得把新条目同步补到 `scripts/i18n-en.txt`，否则 `i18n-build` 会覆盖掉这批手工条目

> v0.4 的完整拆解（签名 / CI / 自动更新 / 崩溃捕获 / 日志跟随 / 帧数统计 / 诊断包升级 / 代理 DLL 覆盖 / CSP / 单实例）见 [`docs/ANR-Studio-v0.4-计划.md`](ANR-Studio-v0.4-计划.md)。

## 附录 T：0.4.0（崩溃捕获 / 自动更新 / 日志跟随 / 诊断包 / 代理 DLL）

> 基线 0.3.0（`86928d1`）。计划见 [`ANR-Studio-v0.4-计划.md`](ANR-Studio-v0.4-计划.md)。
> 本版把「能装、能用」补成「敢让普通用户装、出问题能诊断」。

### T.1 崩溃捕获（P0-4）

| 层 | 做法 |
|---|---|
| 后端 | `crash.rs`：进程启动时 `std::panic::set_hook`，把消息、位置、线程、`Backtrace::force_capture()`、版本、当前视图/最近任务写到 `<数据目录>/crash/crash-<时间戳>.txt`；先写 `.tmp` 再改名，hook 里不调用 Tauri、不做网络 |
| 前端 | `ErrorBoundary` 包住整棵树；`window.onerror` / `unhandledrejection` 通过 `report_frontend_error` 落到同一目录（`kind: frontend`） |
| 提示 | 启动时 `crash_list`，有未处理报告就在标题栏下弹 `CrashNotice`：生成诊断包（crash/*.txt + 系统信息 + `manifest.json`）或忽略（写 `.ack`） |
| 测试 | 报告内容含版本/位置/视图；ack 后不再出现在待处理列表；ack 拒收 crash 目录外的路径 |

`panic = "abort"` 下 hook 仍会在 abort 前执行；hook 内置重入保护，hook 自己 panic 不会递归写文件。
调试构建可用 `ANR_CRASH_TEST=1` 启动，2 秒后主动 `panic!` 验证整条链路。

hook 在进入 Tauri 事件循环**之前**就安装（`pre_setup_data_dir` 用 `%LOCALAPPDATA%` 提前算出目录）：
Tauri 是先创建 WebView 再调用 `setup`，如果 WebView2 初始化失败（典型表现就是「双击没反应」），
setup 根本不会跑，只有提前安装的 hook 才能留下报告。这也验证了本机 WebView2 初始化失败时的落盘。

### T.2 CSP 与外链（P1-9）

- `csp` 从 `null` 收紧为：`default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`。
  保留 `unsafe-inline` 是因为 `vite-plugin-singlefile` 把 JS **内联**进 `index.html`（计划里的方案 A），代价是仍然允许内联脚本，但远程脚本、外连、iframe 注入都被挡住。
- 另配 `devCsp` 放行 `ws://localhost:5173`，避免开发时 HMR 被 CSP 拦掉。
- `open_url` 从 `cmd /C start "" <url>` 改成直接调 `ShellExecuteW`：URL 不再经过 cmd 解析，`?x=1&y=2` 不会被拆成两条命令；只放行 `http(s)`，拒绝控制字符。前端外链统一走 `lib.ts::openExternal`（桌面端只走 `open_url`，浏览器预览才退回 `window.open`）。

### T.3 代理 DLL 手动覆盖（P1-8）

`set_proxy(id, dll)`：只接受 `dxgi.dll` / `winmm.dll` / `version.dll`，先实时复查进程，再按当前版本走一次完整安装：
确认未运行 → 快照 → 准备安装包 → `install.ps1 <dir> <dll>` → 校验 `nr_fg_install.json` 里的 `proxy` 真的等于目标值（不等就回滚）。
抽屉「安装信息」里从只读行改成三段选择 + 每个选项的一句适用说明；切换前弹确认（这是一次重装）。

### T.4 诊断包升级（P1-7）

- 新增 `sysinfo.rs`：PowerShell `Get-CimInstance` 采集 GPU（型号 / 驱动版本 / 驱动日期 / 显存 / 刷新率）、CPU、内存、OS build，以及游戏 exe 的版本资源与大小；WMI 不可用时降级为「未采集到」并写明原因，不让整个诊断包失败。
- 脱敏（只处理 `ini/json/log/txt`，二进制原样）：`C:\Users\<name>` / `D:/Users/<name>` → `%USERPROFILE%`；当前用户名（≥3 字符）出现的位置 → `%USERNAME%`；17/16 位 SteamID64 中间打码。扫描是单遍的（先找 `users` 候选再跳过），几十 MB 日志也不会退化成 O(n²)。
- 诊断包新增 `manifest.json`：工具版本、生成时间、脱敏规则版本、每个文件的字节数与 SHA256（哈希的是脱敏后的内容）。
- 生成后把路径复制到剪贴板，toast 直接给出路径。

### T.5 日志实时跟随（P1-5）

`read_log_since(id, offset, path)`：

- `offset = null`：首次进入，只读文件尾部 256 KB（`reset = true`），不再把整份日志读进内存；
- `offset` 有值：`seek` 后只读增量，单次上限 256 KB；
- 文件变小（截断/轮转）或目标日志路径变了（换游戏）→ `reset = true`。

前端每 1 秒拉一次；`document.visibilityState === 'hidden'` 时停轮询、回来补一次。跟随开关按「内容变化后滚到底部」，用户往上滚就暂停；日志文本在内存里只保留尾部 400 K 字符。手动「重新读取」等于重新走一次 `offset = null`。

### T.6 自动更新（P1-3）

- 依赖 `tauri-plugin-updater 2`；`bundle.createUpdaterArtifacts = true` 让构建同时产出安装包与 `.sig`。
- 更新签名密钥是 minisign（与 Authenticode 两套东西）：`tauri signer generate`，公钥写进 `tauri.conf.json → plugins.updater.pubkey`，私钥只进 CI secrets。
- 更新源默认 GitHub Releases 的 `latest.json`；设置里的「下载源 = 镜像」时改走 `https://gh-proxy.com/<github url>`（`UpdaterBuilder::endpoints`），验签用的是内置公钥，换线路不影响安全。
- 设置 → 关于：「自动 / 仅提示 / 关闭」三档，默认仅提示；启动 3 秒后静默检查，断网失败不弹窗，手动检查才提示。下载进度复用任务中心事件，装好后由 NSIS 覆盖安装并重启。
- 失败兜底：验签失败、下载中断、安装失败都有明确文案，旧版本保留（NSIS per-user 覆盖安装失败不会先删旧目录）。

### T.7 单实例（P2-10）

`tauri-plugin-single-instance` 最先注册：第二个实例在初始化窗口前退出，回调里 `show_main` 把已有窗口从托盘显示出来。避免双开同时写 `settings.json`、同时跑 `rescan`、同时装同一个游戏。

### T.8 签名与 CI（P0-1 / P0-2）

- `.github/workflows/ci.yml`：`npm ci` → `tsc --noEmit` → `vite build` → `cargo test --lib`，Windows runner。
- `.github/workflows/release.yml`：推 `v*` 或手动输入 tag；校验更新私钥存在 → 生成可选的 `tauri.signing.conf.json`（有 Authenticode 证书才写 `signCommand`）→ `tauri-action` 出草稿 Release（NSIS + `.sig` + `latest.json`）→ 生成并上传 `SHA256SUMS.txt`。
- `.github/scripts/sign-windows.ps1`：读 `ANR_WINDOWS_CERT_PFX_BASE64` / `ANR_WINDOWS_CERT_PASSWORD` 或 `ANR_WINDOWS_CERT_THUMBPRINT`，用 `signtool` 做 SHA256 + RFC3161 时间戳；没有证书就跳过（本地产物不签名），不会卡住本地构建。
- `signCommand` 需要绝对路径，所以不写进仓库里的 `tauri.conf.json`，由 CI 生成覆盖配置；这样本机 `tauri build` 不会被固定路径绑死。

### T.9 验证

- `cargo test --lib`：**54 passed / 0 failed / 1 ignored**（新增 crash 4、logs 5、sysinfo 5、lib 2；诊断包用例覆盖脱敏与 manifest 哈希；同时把旧的 `%TEMP%` 测试目录换成仓库内 `target/test-tmp`，受限环境不再误报）
- `cargo check --all-targets`：0 warning
- `node node_modules/typescript/bin/tsc --noEmit` 通过；`vite build` 通过（单文件 537 KB）
- 调试构建 `tauri build --debug --bundles nsis`：配置解析通过，产出 NSIS + `.sig`（更新私钥本地临时注入）
- release 构建（`--bundles nsis`，LTO + strip + `panic=abort`）：`ANR Studio_0.4.0_x64-setup.exe`（2.6 MB）与 `.sig` 均产出
- 界面渲染：本机 WebView2 初始化失败（`0x800700AA`，0.3.0 release 同样复现，属环境问题），改用无头 Edge
  以同样 CSP 渲染 `dist/index.html`：界面正常挂载、`securitypolicyviolation` 为 0，`img-src data:` 封面正常。
- 崩溃链路：同一 WebView2 失败场景下确认 panic hook 被触发并尝试落盘；当前受管环境禁止进程写
  `%LOCALAPPDATA%`（沙箱限制），真实机器上落盘路径由 crash 单测覆盖。
- 手工：诊断包含 GPU/驱动与 `manifest.json`（单测校验脱敏与哈希）；`open_url` 打开 `?x=1&y=2` 正常、`file://` 被拒

### T.10 还没做的

- **帧数 / 延迟统计（P1-6）**：按计划要求等一份真实 `nr_fg-*.log` 样本再写解析器，本版只把日志链路升级为增量读取，不猜字段格式。
- **Authenticode 证书本身**：代码与 CI 已接好，证书/签名服务需要你按计划第 6 节拍板（SignPath / Certum / Azure Trusted Signing）；没有证书时产物仍未签名。
- `latest.json` 的镜像兜底目前复用 `gh-proxy`；若要更稳的国内静态地址，仍是计划里的待定项。

### T.11 运维：更新签名密钥

本机已生成一对开发用密钥（`.tauri-keys/`，已 gitignore）。正式发布前：

1. 把 `.tauri-keys/anr-studio.key` 的内容放进仓库 secret `TAURI_SIGNING_PRIVATE_KEY`；
2. 把 `.tauri-keys/password.txt` 放进 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；
3. 公钥已写入 `tauri.conf.json`；**换密钥必须同时换公钥并发布一次带新公钥的版本**，否则旧版本无法验证新更新包；
4. 私钥/密码不要只留在这台机器上，备份到密码管理器；丢失后无法再给已发布版本推送更新。

### T.12 安装 / 卸载与提权（0.4.0 补丁）

安装包清单是 `asInvoker`，默认装到 `%LOCALAPPDATA%\ANR Studio`，安装和卸载都不需要管理员。之前「装完卸载不掉」的两个真实原因：

1. 应用关闭窗口只是收进托盘，进程还在；NSIS 的卸载检查拿不到它，卸载直接失败。
2. 如果安装包或应用曾经以管理员身份运行过，普通权限的卸载程序既看不到也杀不掉那个提权进程。

修法：

- 卸载钩子 `NSIS_HOOK_PREUNINSTALL`：先 `taskkill`（正常结束 → 强制），再对 `anr-studio.exe` 做写探测；仍然不行就带 `/ANR_ELEVATED` 用 `ExecShell "runas"` 重新拉起卸载程序（保留 `/UPDATE` / `/S` / `/P`），已经提权过还失败则给出「手动删除 + HKCU 卸载项」的明确提示，不进入 UAC 死循环。
- 安装钩子的提示从「去用管理员运行」改成「推荐退回默认的用户目录；必须装受保护目录才需要管理员，且以后卸载也会需要」。
- 安装完成后「运行 ANR Studio」不再默认勾选（`MUI_FINISHPAGE_RUN_NOTCHECKED`）：避免提权安装时把应用也以管理员权限拉起来。
- 更新检查遇到 404（`latest.json` 还没上传）映射为「还没有可用的更新信息：GitHub Releases 上尚未发布 latest.json」，不再显示成故障。

验证：`cargo test --lib` **54 passed**；重建 NSIS 成功，生成的安装包带新卸载器与更新包 `.sig`。

> 注：已经安装在 `D:\SoftWare\ANR Studio` 的旧副本，用新安装包覆盖安装后会换上新的 `uninstall.exe`；之后卸载即可正常结束进程。旧目录不会自动迁移，想换默认目录请在安装向导里改。
