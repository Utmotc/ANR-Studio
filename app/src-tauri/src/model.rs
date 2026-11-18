use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NrFgConfig {
    pub path: String,
    pub model_size: String,
    pub nits: u32,
    pub max_generated: u8,
    pub scene_cut: bool,
}

impl Default for NrFgConfig {
    fn default() -> Self {
        Self {
            path: "auto".into(),
            model_size: "large".into(),
            nits: 80,
            max_generated: 5,
            scene_cut: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnrState {
    pub kind: String,
    pub version: String,
    pub proxy: String,
    pub installed_at: String,
    pub enabled: bool,
    /// OptiScaler.ini 里 [FrameGen] FGNvngxReplacement 的当前值
    pub fg_replacement: String,
    pub fingerprint: String,
    pub config: NrFgConfig,
    pub health: String,
    pub health_note: Option<String>,
    pub log_path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub source: String,
    pub dir: String,
    pub exe: String,
    pub last_used: String,
    #[serde(rename = "sizeGB")]
    pub size_gb: f64,
    pub running: bool,
    #[serde(rename = "thirdPartyOptiScaler")]
    pub third_party_optiscaler: bool,
    pub anr: Option<AnrState>,
    #[serde(default)]
    pub cover: Option<String>,
    /// 5.1 状态矩阵里的「需人工确认」异常态；非空时安装/卸载要先确认
    #[serde(default)]
    pub conflict: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub tag: String,
    pub product: String,
    pub published_at: String,
    #[serde(rename = "sizeMB")]
    pub size_mb: f64,
    pub cached: bool,
    pub latest: bool,
    pub fingerprint: String,
    pub notes: String,
}

fn default_density() -> String {
    "cozy".into()
}

fn default_font_size() -> u8 {
    2
}

fn default_cover_source() -> String {
    "auto".into()
}

fn default_startup_view() -> String {
    "overview".into()
}

fn default_close_action() -> String {
    "ask".into()
}

fn default_concurrency() -> u32 {
    1
}

fn default_locale() -> String {
    "zh".into()
}

fn default_layout() -> String {
    "grid".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    #[serde(default = "default_density")]
    pub density: String,
    #[serde(default = "default_font_size")]
    pub font_size: u8,
    /// 是否已经看过首次启动引导
    #[serde(default)]
    pub onboarded: bool,
    /// 封面来源：auto / domestic / global
    #[serde(default = "default_cover_source")]
    pub cover_source: String,
    #[serde(default = "default_startup_view")]
    pub startup_view: String,
    #[serde(default)]
    pub reduce_motion: bool,
    #[serde(default = "default_close_action")]
    pub close_action: String,
    pub mirror: String,
    pub cache_dir: String,
    pub warn_anti_cheat: bool,
    pub keep_snapshot: bool,
    /// 同时进行的安装/下载任务数，1 = 严格串行（6.5.4）
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    /// 默认安装版本；空 = 跟随最新版本（6.4④「设为默认」）
    #[serde(default)]
    pub default_version: String,
    /// 界面语言：zh / en
    #[serde(default = "default_locale")]
    pub locale: String,
    /// 游戏库视图：grid / list（6.4② 记住选择）
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default)]
    pub manual_games: Vec<String>,
    /// 从列表里移除掉的扫描游戏（只隐藏，不动文件）
    #[serde(default)]
    pub hidden_games: Vec<String>,
    /// 用户自定义的游戏显示名，键是游戏 id（目录哈希）
    #[serde(default)]
    pub renames: std::collections::HashMap<String, String>,
    /// 上次运行时的版本，用来判断是否需要刷新系统图标缓存
    #[serde(default)]
    pub last_version: String,
}

impl Settings {
    pub fn with_defaults(cache_dir: String) -> Self {
        Self {
            theme: "dark".into(),
            density: "cozy".into(),
            font_size: 2,
            onboarded: false,
            cover_source: "auto".into(),
            startup_view: "overview".into(),
            reduce_motion: false,
            close_action: "ask".into(),
            mirror: "auto".into(),
            cache_dir,
            warn_anti_cheat: true,
            keep_snapshot: true,
            concurrency: 1,
            default_version: String::new(),
            locale: "zh".into(),
            layout: "grid".into(),
            manual_games: Vec::new(),
            hidden_games: Vec::new(),
            renames: std::collections::HashMap::new(),
            last_version: String::new(),
        }
    }
}

/// 卸载/切换前生成的快照。schema=0 表示 0.2.2 及更早只含 OptiScaler 目录的旧快照。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub path: String,
    pub file_name: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub game_id: String,
    pub game_name: String,
    pub schema: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub stage: String,
    pub progress: f64,
    pub status: String,
}
