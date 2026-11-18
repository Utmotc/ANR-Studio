//! OptiScaler.ini 的读写，以及「照抄游戏内菜单」的结构模型。
//!
//! 为什么是菜单模型而不是 ini 分节模型：游戏里的面板把参数按
//! Upscalers / Sharpness / Output Scaling / Frame Generation / Framerate ...
//! 分组，和 ini 的分节不是一一对应的（比如 Sharpness 在 ini 里是独立分节，
//! 菜单里归到 Upscalers 下面）。这里以菜单为准，ini 只是存储。
//!
//! 约束：OptiScaler 只在启动时读 ini、退出时写回，所以 App 只能「改了等重启」。

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::ini::IniDoc;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub tab: String,
    pub group_cn: String,
    pub group_en: String,
    /// ini 分节（写回用；status 行为空）
    pub section: String,
    pub key: String,
    pub label: String,
    pub label_en: String,
    pub hint: String,
    pub value: String,
    /// toggle | select | slider | number | text | status
    pub kind: String,
    pub options: Vec<String>,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub known: bool,
    pub readonly: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    pub id: String,
    pub cn: String,
    pub en: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tabs: Vec<TabInfo>,
    pub fields: Vec<Field>,
    pub running: bool,
    pub path: String,
    pub mtime: u64,
}

// ───────────────────────────── 菜单骨架 ─────────────────────────────

struct Tab {
    id: &'static str,
    cn: &'static str,
    en: &'static str,
}

static TABS: &[Tab] = &[
    Tab { id: "upscaling", cn: "上采样", en: "Upscaling" },
    Tab { id: "framegen", cn: "帧生成", en: "Frame Gen" },
    Tab { id: "latency", cn: "延迟", en: "Latency" },
    Tab { id: "textures", cn: "纹理", en: "Textures" },
    Tab { id: "compat", cn: "兼容", en: "Compatibility" },
    Tab { id: "menu", cn: "菜单", en: "Menu" },
    Tab { id: "status", cn: "状态", en: "Status" },
];

struct Item {
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
    kind: &'static str,
    opts: &'static [&'static str],
    min: f64,
    max: f64,
    step: f64,
}

const fn base(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
    kind: &'static str,
) -> Item {
    Item { tab, gcn, gen, section, key, cn, en, hint, kind, opts: &[], min: 0.0, max: 0.0, step: 0.0 }
}

/// 开关（三态：开 / 关 / 自动）
const fn t(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
) -> Item {
    base(tab, gcn, gen, section, key, cn, en, hint, "toggle")
}

/// 下拉
const fn s(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
    opts: &'static [&'static str],
) -> Item {
    Item { opts, ..base(tab, gcn, gen, section, key, cn, en, hint, "select") }
}

/// 滑块
const fn r(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
    min: f64,
    max: f64,
    step: f64,
) -> Item {
    Item { min, max, step, ..base(tab, gcn, gen, section, key, cn, en, hint, "slider") }
}

/// 数字输入
const fn n(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
) -> Item {
    base(tab, gcn, gen, section, key, cn, en, hint, "number")
}

/// 文本输入
const fn x(
    tab: &'static str,
    gcn: &'static str,
    gen: &'static str,
    section: &'static str,
    key: &'static str,
    cn: &'static str,
    en: &'static str,
    hint: &'static str,
) -> Item {
    base(tab, gcn, gen, section, key, cn, en, hint, "text")
}

const UP: &[&str] = &["auto", "fsr2", "fsr3", "xess", "dlss", "ffx"];
const FG_IN: &[&str] = &["auto", "DLSSG", "NvngxFG", "FSRFG", "Upscaler", "FSRFG30", "XeFG", "NoFG"];
const FG_NVNGX: &[&str] = &[
    "Apex Neural Rendering FG",
    "None",
    "Nukems",
    "Arturs",
    "FFX",
    "Combo",
];
const SHADER: &[&str] = &["auto", "rcas", "cas"];
const LOWLAT: &[&str] = &["auto", "Reflex", "LatencyFlex", "XeLL", "None"];
const ANISO: &[&str] = &["auto", "2", "4", "8", "16"];

/// 顺序 = 游戏里从上到下的顺序
static ITEMS: &[Item] = &[
    // ══════════ Upscaling ══════════
    s("upscaling", "超分器", "Upscalers", "Upscalers", "Dx12Upscaler", "DX12 超分器", "DX12 Upscaler", "游戏用 DX12 时用哪套超分，绝大多数游戏看这项", UP),
    s("upscaling", "超分器", "Upscalers", "Upscalers", "Dx11Upscaler", "DX11 超分器", "DX11 Upscaler", "游戏用 DX11 时的超分实现", UP),
    s("upscaling", "超分器", "Upscalers", "Upscalers", "VulkanUpscaler", "Vulkan 超分器", "Vulkan Upscaler", "Vulkan 游戏用的超分实现", UP),
    // Sharpness（菜单里挂在 Upscalers 下面）
    t("upscaling", "锐化", "Sharpness", "Sharpness", "OverrideSharpness", "覆盖锐化", "Override", "打开后忽略游戏自带的锐化设置"),
    r("upscaling", "锐化", "Sharpness", "Sharpness", "Sharpness", "锐化强度", "Strength", "0 关，0.4 左右自然，超过 0.8 会发脏", 0.0, 1.0, 0.05),
    s("upscaling", "锐化", "Sharpness", "Sharpness", "Shader", "锐化方式", "Method", "不同的锐化实现，RCAS 最稳", SHADER),
    // Upscale Ratio Override
    t("upscaling", "分辨率比例覆盖", "Upscale Ratio Override", "UpscaleRatio", "UpscaleRatioOverrideEnabled", "覆盖全部分辨率比例", "Override all", "不用游戏给的档位，统一按下面的比例渲染"),
    t("upscaling", "分辨率比例覆盖", "Upscale Ratio Override", "QualityOverrides", "QualityRatioOverrideEnabled", "按质量档分别覆盖", "Override per quality preset", "每一档单独指定比例"),
    r("upscaling", "分辨率比例覆盖", "Upscale Ratio Override", "UpscaleRatio", "UpscaleRatioOverrideValue", "比例", "Ratio", "1.5 表示按 1.5 倍渲染：画质更好、帧数更低", 1.0, 3.0, 0.01),
    r("upscaling", "分辨率比例覆盖", "Upscale Ratio Override", "QualityOverrides", "QualityRatioQuality", "质量档比例", "Quality", "质量档的渲染比例", 1.0, 3.0, 0.01),
    r("upscaling", "分辨率比例覆盖", "Upscale Ratio Override", "QualityOverrides", "QualityRatioPerformance", "性能档比例", "Performance", "性能档的渲染比例", 1.0, 3.0, 0.01),
    // Output Scaling
    t("upscaling", "输出缩放", "Output Scaling", "OutputScaling", "Enabled", "启用输出缩放", "Enable", "把渲染结果再缩放到输出分辨率"),
    s("upscaling", "输出缩放", "Output Scaling", "OutputScaling", "Downscaler", "降采样方式", "Downscaler", "输出分辨率更低时用的算法", &["auto", "fsr1"]),
    r("upscaling", "输出缩放", "Output Scaling", "OutputScaling", "Multiplier", "缩放倍数", "Ratio", "2.0 表示以两倍分辨率渲染再缩回来", 1.0, 3.0, 0.01),
    // Magnifier
    t("upscaling", "放大镜", "Magnifier", "Magnifier", "Enabled", "启用放大镜", "Enabled", "调试用，游戏里放大局部看细节"),
    n("upscaling", "放大镜", "Magnifier", "Magnifier", "Size", "放大区域", "Size", "放大镜覆盖的区域大小"),
    n("upscaling", "放大镜", "Magnifier", "Magnifier", "ZoomFactor", "放大倍数", "Zoom Factor", "看细节时调大"),
    // ══════════ Frame Gen ══════════
    t("framegen", "帧生成", "Frame Generation", "FrameGen", "Enabled", "帧生成开关", "Frame Generation", "auto 跟随游戏里的帧生成选项"),
    s("framegen", "帧生成", "Frame Generation", "FrameGen", "FGInput", "生成输入", "FG Input", "DLSSG 要求游戏本身支持；Upscaler 用超分链路喂数据", FG_IN),
    s("framegen", "帧生成", "Frame Generation", "FrameGen", "FGNvngxReplacement", "生成实现", "FG Nvngx", "选 Apex Neural Rendering FG 就是 ANR", FG_NVNGX),
    t("framegen", "帧生成", "Frame Generation", "FrameGen", "DisableHudless", "禁用无 HUD 渲染", "Disable Hudless", "HUD 闪烁或黑块时打开"),
    t("framegen", "帧生成", "Frame Generation", "DLSSG", "OverrideInterpolationCount", "强制生成帧数", "Override DLSSG Ratio", "忽略游戏给的倍数，用下面这个值"),
    r("framegen", "帧生成", "Frame Generation", "DLSSG", "InterpolationCount", "生成帧数", "Interpolation Count", "每两帧真实画面之间插几张：2 = 3 倍，5 = 6 倍", 0.0, 5.0, 1.0),
    t("framegen", "帧生成", "Frame Generation", "DLSSG", "ForceDMFG", "动态多帧生成", "Force Dynamic MFG", "帧率不足时自动多插帧"),
    r("framegen", "帧生成", "Frame Generation", "DLSSG", "FramerateTargetDMFG", "DMFG 目标帧率", "DMFG FPS Target", "动态多帧生成瞄准的帧率", 30.0, 240.0, 5.0),
    s("framegen", "帧生成", "Frame Generation", "FrameGen", "FTInput", "帧时间来源", "Frametime Input", "影响补帧节奏，一般留 auto", &["auto", "0", "1", "2"]),
    // Framerate
    n("framegen", "帧率", "Framerate", "Framerate", "FramerateLimit", "帧率上限", "FPS Limit", "0 表示不限制；填 inf 也可以"),
    // ══════════ Latency ══════════
    t("latency", "低延迟", "Low Latency", "fakenvapi", "UseFakenvapi", "启用 fakenvapi", "Use Fakenvapi", "给非 N 卡补上 Reflex 接口"),
    s("latency", "低延迟", "Low Latency", "fakenvapi", "LowLatencyInput", "输入延迟接口", "Input", "接管游戏的输入侧低延迟", LOWLAT),
    s("latency", "低延迟", "Low Latency", "fakenvapi", "LowLatencyOutput", "输出延迟接口", "Output", "接管游戏的输出侧低延迟", LOWLAT),
    t("latency", "低延迟", "Low Latency", "fakenvapi", "ForceReflex", "强制 Reflex", "Force Reflex", "强制开启 NVIDIA Reflex"),
    t("latency", "低延迟", "Low Latency", "fakenvapi", "ForceLatencyFlex", "强制 LatencyFlex", "Force LatencyFlex", "用 LatencyFlex 代替 Reflex"),
    t("latency", "低延迟", "Low Latency", "fakenvapi", "ForceXeLL", "强制 XeLL", "Force XeLL", "Intel 的低延迟接口"),
    // V-Sync
    t("latency", "垂直同步", "V-Sync Settings", "V-Sync", "OverrideVsync", "覆盖垂直同步", "Override Vsync", "接管游戏的垂直同步设置"),
    s("latency", "垂直同步", "V-Sync Settings", "V-Sync", "ForceVsync", "垂直同步", "V-Sync", "开能防撕裂但增加延迟", &["auto", "true", "false"]),
    s("latency", "垂直同步", "V-Sync Settings", "V-Sync", "SyncInterval", "同步间隔", "Sync Int.", "0 表示立即呈现，数值越大越省电", &["auto", "0", "1", "2", "3", "4"]),
    // ══════════ Textures ══════════
    t("textures", "Mipmap 偏移", "Mipmap Bias", "Mipmap", "MipmapBiasOverride", "启用 Mipmap 偏移", "Mipmap Bias", "贴图模糊或闪烁时调"),
    r("textures", "Mipmap 偏移", "Mipmap Bias", "Mipmap", "MipmapBiasFixedOverride", "固定偏移", "MB Fixed Override", "直接指定偏移值，负值更清晰", -2.0, 2.0, 0.01),
    r("textures", "Mipmap 偏移", "Mipmap Bias", "Mipmap", "MipmapBiasScaleOverride", "按比例偏移", "MB Scale Override", "按分辨率比例缩放偏移", 0.0, 1.0, 0.01),
    t("textures", "Mipmap 偏移", "Mipmap Bias", "Mipmap", "MipmapBiasOverrideAll", "应用到全部贴图", "MB Override All Textures", "不勾只影响部分贴图"),
    // Anisotropic Filtering
    s("textures", "各向异性过滤", "Anisotropic Filtering", "Anisotropy", "AnisotropyOverride", "强制各向异性过滤", "Force Anisotropic Filtering", "覆盖游戏里的各向异性设置", ANISO),
    t("textures", "各向异性过滤", "Anisotropic Filtering", "Anisotropy", "ModifyComparison", "修改比较采样", "Modify Compare", "贴图边缘发虚时试"),
    t("textures", "各向异性过滤", "Anisotropic Filtering", "Anisotropy", "ModifyMinMax", "修改 Min/Max", "Modify Min/Max", "同时改写最小/最大 LOD"),
    t("textures", "各向异性过滤", "Anisotropic Filtering", "Anisotropy", "SkipPointFilter", "跳过点采样", "Skip Point Filters", "某些游戏需要跳过点采样才正常"),
    // HDR
    t("textures", "HDR", "HDR", "HDR", "ForceHDR", "强制 HDR", "Force HDR", "游戏不认 HDR 时打开"),
    t("textures", "HDR", "HDR", "HDR", "UseHDR10", "使用 HDR10", "Use HDR10", "用 PQ 曲线而不是 sRGB"),
    // ══════════ Compatibility ══════════
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "EnableFsr2Inputs", "FSR2 启用", "FSR2 · Enable", "允许复用 FSR2 的输入数据"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "UseFsr2Inputs", "FSR2 使用", "FSR2 · Use", "实际采用 FSR2 的输入"),
    x("compat", "超分输入", "Upscaler Inputs", "Inputs", "Fsr2Pattern", "FSR2 匹配串", "FSR2 · Pattern", "匹配 dll 名的通配符，留 auto 即可"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "EnableFsr3Inputs", "FSR3 启用", "FSR3 · Enable", "允许复用 FSR3 的输入数据"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "UseFsr3Inputs", "FSR3 使用", "FSR3 · Use", "实际采用 FSR3 的输入"),
    x("compat", "超分输入", "Upscaler Inputs", "Inputs", "Fsr3Pattern", "FSR3 匹配串", "FSR3 · Pattern", "匹配 dll 名的通配符"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "EnableFfxInputs", "FFX 启用", "FFX · Enable", "允许复用 FFX 的输入数据"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "UseFfxInputs", "FFX 使用", "FFX · Use", "实际采用 FFX 的输入"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "EnableDlssInputs", "DLSS 启用", "DLSS · Enable", "允许复用 DLSS 的输入数据"),
    t("compat", "超分输入", "Upscaler Inputs", "Inputs", "EnableHotSwapping", "热切换", "Hot Swapping", "游戏运行中换超分实现"),
    // Startup
    t("compat", "启动", "Startup", "Plugins", "LoadAsiPlugins", "加载 ASI 插件", "Load ASI plugins", "兼容 Ultimate ASI Loader 那类插件"),
    t("compat", "启动", "Startup", "Menu", "DisableSplash", "关闭启动提示", "Disable Splash message", "进游戏不再弹 OptiScaler 的提示条"),
    t("compat", "启动", "Startup", "Hotfix", "CheckForUpdate", "检查更新", "Check for Update", "OptiScaler 自己联网查新版本"),
    // Shaders
    t("compat", "着色器", "Shaders", "Hotfix", "UsePrecompiledShaders", "使用预编译着色器", "Use Precompiled Shaders", "减少首次进入时的卡顿"),
    t("compat", "着色器", "Shaders", "DRS", "DrsMinOverrideEnabled", "动态分辨率下限", "DRS Override Minimum", "锁住动态分辨率的下限"),
    t("compat", "着色器", "Shaders", "DRS", "DrsMaxOverrideEnabled", "动态分辨率上限", "DRS Override Maximum", "锁住动态分辨率的上限"),
    // Hooks
    t("compat", "注入钩子", "Hooks", "Hooks", "EarlyHooking", "提前注入", "Early Hooking", "游戏加载很早时打开，能提高成功率"),
    t("compat", "注入钩子", "Hooks", "Hooks", "UseNtdllHooks", "用 ntdll 钩子", "Use Ntdll Hooks", "常规钩子被拦时的备选"),
    t("compat", "注入钩子", "Hooks", "Hooks", "HookOriginalNvngxOnly", "只钩原始 nvngx", "Hook Original Nvngx Only", "避免和其他注入工具互相干扰"),
    // Plugins
    t("compat", "插件", "Plugins", "Plugins", "LoadReshade", "加载 ReShade", "Load ReShade", "同目录有 ReShade 时一起加载"),
    t("compat", "插件", "Plugins", "Plugins", "LoadSpecialK", "加载 Special K", "Load Special K", "同目录有 Special K 时一起加载"),
    // Spoofing
    x("compat", "伪装", "Spoofing", "Spoofing", "SpoofedGPUName", "伪装显卡名", "Spoofed GPU Name", "显示给游戏的显卡型号"),
    t("compat", "伪装", "Spoofing", "Spoofing", "StreamlineSpoofing", "Streamline 伪装", "Streamline Spoofing", "让游戏以为有完整 Streamline 支持"),
    // Hotfix
    t("compat", "兼容修补", "Hotfix", "Hotfix", "DisableOverlays", "禁用其它覆盖层", "Disable Overlays", "和 Steam / Discord 覆盖层打架时打开"),
    t("compat", "兼容修补", "Hotfix", "Hotfix", "PreferDedicatedGpu", "优先独显", "Prefer Dedicated GPU", "双显卡笔记本很有用"),
    t("compat", "兼容修补", "Hotfix", "Hotfix", "SimulateWaitableObject", "模拟可等待对象", "Simulate Waitable Object", "某些游戏卡启动时试"),
    // ══════════ Menu ══════════
    s("menu", "菜单", "Menu", "Menu", "Scale", "菜单缩放", "Menu Scale", "游戏内菜单太小或太大时调", &["auto", "1", "1.25", "1.5", "2"]),
    t("menu", "菜单", "Menu", "Menu", "OverlayMenu", "启用覆盖层菜单", "Overlay Menu", "游戏里按 Insert 调出菜单"),
    t("menu", "菜单", "Menu", "Menu", "UseHQFont", "高质量字体", "Use HQ Font", "字体更清晰，稍微多吃性能"),
    // FPS Overlay
    t("menu", "帧率叠加", "FPS Overlay", "Menu", "ShowFps", "显示帧率", "FPS Overlay Enabled", "游戏里叠加显示帧率和曲线"),
    t("menu", "帧率叠加", "FPS Overlay", "Menu", "FpsOverlayHorizontal", "横向排列", "Horizontal", "叠加快关横过来显示"),
    s("menu", "帧率叠加", "FPS Overlay", "Menu", "FpsOverlayPos", "位置", "Overlay Position", "叠加层贴在屏幕哪个角", &["auto", "TopLeft", "TopRight", "BottomLeft", "BottomRight"]),
    s("menu", "帧率叠加", "FPS Overlay", "Menu", "FpsOverlayType", "样式", "Overlay Type", "纯数字还是带曲线", &["auto", "Full", "FullGraph", "Simple"]),
    r("menu", "帧率叠加", "FPS Overlay", "Menu", "FpsScale", "叠加缩放", "Scale", "叠加层的字号", 0.5, 2.0, 0.1),
    // Keybinds
    x("menu", "快捷键", "Keybinds", "Menu", "ShortcutKey", "菜单快捷键", "Menu", "默认 Insert，可改成 F1~F12 之类"),
    x("menu", "快捷键", "Keybinds", "Menu", "FpsShortcutKey", "帧率叠加快捷键", "FPS Overlay", "显示/隐藏帧率叠加"),
    x("menu", "快捷键", "Keybinds", "Menu", "FpsCycleShortcutKey", "切换叠加样式", "FPS Overlay Cycle", "在几种样式之间轮换"),
    // Logging
    t("menu", "日志", "Logging", "Log", "LogToFile", "写日志文件", "Log To File", "游戏目录下生成 OptiScaler.log"),
    t("menu", "日志", "Logging", "Log", "LogToConsole", "输出到控制台", "To Console", "调试时开"),
    s("menu", "日志", "Logging", "Log", "LogLevel", "日志级别", "Log Level", "0 最啰嗦，4 只记错误", &["0", "1", "2", "3", "4"]),
];

fn tab_exists(id: &str) -> bool {
    TABS.iter().any(|t| t.id == id)
}

fn group_order(tab: &str, gcn: &str) -> usize {
    ITEMS
        .iter()
        .filter(|i| i.tab == tab)
        .position(|i| i.gcn == gcn)
        .unwrap_or(usize::MAX)
}

// ───────────────────────────── 读写 ─────────────────────────────

fn parse_ini(text: &str) -> (Vec<String>, HashMap<String, (String, String)>) {
    // 返回 (分节顺序, "分节\u{1}键" -> (值, 分节))
    let mut sections: Vec<String> = Vec::new();
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    let mut current = String::new();
    for line in text.lines() {
        let s = line.trim();
        if s.is_empty() || s.starts_with(';') || s.starts_with('#') {
            continue;
        }
        if s.starts_with('[') && s.ends_with(']') {
            current = s[1..s.len() - 1].to_string();
            if !sections.contains(&current) {
                sections.push(current.clone());
            }
            continue;
        }
        let Some((k, v)) = s.split_once('=') else { continue };
        let key = k.trim().to_string();
        if current.is_empty() || key.is_empty() {
            continue;
        }
        map.insert(
            format!("{current}\u{1}{key}"),
            (v.trim().to_string(), current.clone()),
        );
    }
    (sections, map)
}

fn norm(v: &str) -> &str {
    let t = v.trim();
    if t.is_empty() {
        "auto"
    } else {
        t
    }
}


/// 状态页：只读信息，对照游戏内 Status 面板
fn status_rows(dir: &Path) -> Vec<Field> {
    let mut out = Vec::new();
    let mut push = |k: &str, cn: &str, en: &str, v: String| {
        out.push(Field {
            tab: "status".into(),
            group_cn: "系统".into(),
            group_en: "System".into(),
            section: "Status".into(),
            key: k.to_string(),
            label: cn.to_string(),
            label_en: en.to_string(),
            hint: String::new(),
            value: v,
            kind: "status".into(),
            options: Vec::new(),
            min: 0.0,
            max: 0.0,
            step: 0.0,
            known: true,
            readonly: true,
        });
    };

    push("Dir", "游戏目录", "Game directory", dir.to_string_lossy().to_string());

    let ini = crate::inspect::ini_path(dir);
    push("Ini", "配置文件", "OptiScaler.ini", ini.to_string_lossy().to_string());

    // 代理 DLL：以安装记录为准
    let rec_p = crate::inspect::record_path(dir);
    if let Ok(txt) = fs::read_to_string(&rec_p) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            let proxy = v.get("proxy").and_then(|x| x.as_str()).unwrap_or("未知");
            let at = v.get("installed_at").and_then(|x| x.as_str()).unwrap_or("");
            push("Proxy", "代理 DLL", "Proxy DLL", proxy.to_string());
            if !at.is_empty() {
                push("Installed", "安装时间", "Installed at", at.to_string());
            }
        }
    } else {
        push("Proxy", "代理 DLL", "Proxy DLL", "没有安装记录".into());
    }

    // provider
    let provider = crate::inspect::provider_dll(dir);
    push(
        "Provider",
        "提供者",
        "Provider",
        if provider.exists() {
            "Apex Neural Rendering FG".to_string()
        } else {
            "缺失".to_string()
        },
    );

    let log = dir.join("OptiScaler.log");
    push(
        "Log",
        "日志文件",
        "OptiScaler.log",
        if log.exists() {
            "已生成".to_string()
        } else {
            "未生成（游戏还没启动过，或日志被关）".to_string()
        },
    );

    out
}

pub fn read(dir: &Path) -> Result<Snapshot, String> {
    let path = crate::inspect::ini_path(dir);
    if !path.exists() {
        return Err("这个目录里没有 OptiScaler.ini，先安装 ANR".into());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 OptiScaler.ini 失败：{e}"))?;
    let (sections, map) = parse_ini(&text);

    let mut fields: Vec<Field> = Vec::new();
    let mut used: Vec<String> = Vec::new();

    for it in ITEMS {
        let key_id = format!("{}\u{1}{}", it.section, it.key);
        let value = map.get(&key_id).map(|(v, _)| v.clone()).unwrap_or_default();
        used.push(key_id);

        let mut options: Vec<String> = it.opts.iter().map(|s| s.to_string()).collect();
        // 当前值不在候选里时补进去，避免下拉显示空白
        let cur = norm(&value);
        if it.kind == "select" && !cur.is_empty() && !options.iter().any(|o| o.eq_ignore_ascii_case(cur)) {
            options.push(cur.to_string());
        }

        fields.push(Field {
            tab: it.tab.to_string(),
            group_cn: it.gcn.to_string(),
            group_en: it.gen.to_string(),
            section: it.section.to_string(),
            key: it.key.to_string(),
            label: it.cn.to_string(),
            label_en: it.en.to_string(),
            hint: it.hint.to_string(),
            value: if value.is_empty() { "auto".into() } else { value },
            kind: it.kind.to_string(),
            options,
            min: it.min,
            max: it.max,
            step: it.step,
            known: true,
            readonly: false,
        });
    }

    // 没进菜单的键：按 ini 分节挂到对应 Tab 末尾
    for sec in &sections {
        let keys: Vec<(&String, &(String, String))> =
            map.iter().filter(|(k, v)| &v.1 == sec && k.starts_with(&format!("{sec}\u{1}"))).collect();
        for (k, (v, _)) in keys {
            if used.iter().any(|u| u == k) {
                continue;
            }
            let key = k.split('\u{1}').nth(1).unwrap_or("").to_string();
            let tab = tab_of_section(sec);
            fields.push(Field {
                tab: tab.to_string(),
                group_cn: format!("{} · 其它", sec),
                group_en: format!("{} · Others", sec),
                section: sec.clone(),
                key: key.clone(),
                label: String::new(),
                label_en: key,
                hint: String::new(),
                value: v.clone(),
                kind: infer_kind(v).to_string(),
                options: Vec::new(),
                min: 0.0,
                max: 0.0,
                step: 0.0,
                known: false,
                readonly: false,
            });
        }
    }

    // 组内顺序 = ITEMS 出现顺序；跨组按首次出现
    fields.sort_by_key(|f| {
        (
            TABS.iter().position(|t| t.id == f.tab).unwrap_or(99),
            group_order(&f.tab, &f.group_cn),
            if !f.known { 1 } else { 0 },
        )
    });

    // 状态页是只读信息，不参与排序
    fields.extend(status_rows(dir));

    let mtime = fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(Snapshot {
        tabs: TABS
            .iter()
            .map(|t| TabInfo {
                id: t.id.to_string(),
                cn: t.cn.to_string(),
                en: t.en.to_string(),
            })
            .collect(),
        fields,
        running: false,
        path: path.to_string_lossy().to_string(),
        mtime,
    })
}

fn tab_of_section(sec: &str) -> &'static str {
    match sec {
        "Upscalers" | "Sharpness" | "CAS" | "OutputScaling" | "UpscaleRatio"
        | "QualityOverrides" | "FSR" | "DLSS" | "DLSSD" | "XeSS" | "InitFlags" | "Inputs"
        | "Reprojection" => "upscaling",
        "FrameGen" | "OptiFG" | "DLSSG" | "FSRFG" | "XeFG" | "NvngxFG" | "FSRFGInputs"
        | "Framerate" | "V-Sync" => "framegen",
        "fakenvapi" | "NvApi" => "latency",
        "Mipmap" | "Anisotropy" | "HDR" => "textures",
        "Hooks" | "Libraries" | "Plugins" | "Spoofing" | "Hotfix" | "Dx11withDx12"
        | "ProcessFilter" | "DRS" => "compat",
        "Menu" | "Log" => "menu",
        _ => {
            if tab_exists("status") {
                "status"
            } else {
                "menu"
            }
        }
    }
}

fn infer_kind(v: &str) -> &'static str {
    let lower = v.trim().to_ascii_lowercase();
    if matches!(lower.as_str(), "true" | "false" | "auto") {
        return "toggle";
    }
    if v.trim().parse::<f64>().is_ok() {
        return "number";
    }
    "text"
}

pub fn write(dir: &Path, section: &str, key: &str, value: &str) -> Result<String, String> {
    let path = crate::inspect::ini_path(dir);
    if !path.exists() {
        return Err("找不到 OptiScaler.ini".into());
    }
    if value.contains('\n') || value.contains('\r') {
        return Err("值里不能有换行".into());
    }
    let value = value.trim();
    if value.len() > 200 {
        return Err("值太长了".into());
    }

    // 有定义的项按类型校验
    if let Some(it) = ITEMS
        .iter()
        .find(|i| i.section.eq_ignore_ascii_case(section) && i.key.eq_ignore_ascii_case(key))
    {
        match it.kind {
            "toggle" => {
                if !matches!(value.to_ascii_lowercase().as_str(), "true" | "false" | "auto") {
                    return Err("这个开关只能是 开 / 关 / 自动".into());
                }
            }
            "slider" | "number" => {
                if value.parse::<f64>().is_err() && !value.eq_ignore_ascii_case("auto") {
                    return Err("这里要填数字（或者 auto）".into());
                }
            }
            "select" => {
                if !it.opts.is_empty()
                    && !it.opts.iter().any(|o| o.eq_ignore_ascii_case(value))
                    && !value.eq_ignore_ascii_case("auto")
                {
                    return Err(format!("只能是 {}", it.opts.join(" / ")));
                }
            }
            _ => {}
        }
    }

    let mut doc = IniDoc::load(&path);
    IniDoc::backup_once(&path);
    doc.set(section, key, value);
    doc.save(&path)
        .map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
    Ok(value.to_string())
}
