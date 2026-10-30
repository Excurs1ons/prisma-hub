//! 新建工程的能力目录：界面上每一项的说明、利弊与体积成本只在这里定义一次。
//!
//! 目录是**展示 + 估算数据**，不参与推导：真正决定 `prism-app` feature 的仍然是
//! [`crate::projects::ProjectCapabilities::features`]。两边靠 `id` 对应：
//! [`crate::projects::ProjectCapabilities::option_state`] /
//! [`crate::projects::ProjectCapabilities::apply_option`] 认识哪些 `id`，目录里的
//! `enabled` 必须与之一致（测试 `every_catalog_id_is_recognised_by_the_derivation`
//! 锁住这一点）。
//!
//! ## 体积数据
//!
//! 全部来自 PrismaRev 引擎侧实测（`dist` profile、strip、Linux aarch64）：
//!
//! - 基准构建 = 3D 全量 + x11 + wayland + ui ≈ 4.47 MB（[`CapabilityCatalog::baseline_kb`]）；
//! - 每项的 `size_delta_kb` 是「相对基准构建、把这一项切到**非基准状态**」时的体积
//!   变化（负 = 变小），`baseline_on` 说明基准构建里它是开还是关。所以它不总是
//!   `cost` 原文的复述：`wayland` 的 `cost` 写的是它的重量（约 +1.1 MB），而基准里
//!   Wayland 是开的，关掉它 `size_delta_kb` 就是 −1100；
//! - 界面把已偏离基准的项累加成「预计体积」。单项实测值相加不是严格线性的，所以那里
//!   一律标「估算」，`size_delta_kb: None` 的项（未接线 / 未实测）不参与累加。
//!
//! 数字宁可写「未实测」也不要估算完当实测：`audio` / `wayland` 的「约」、
//! `physics` 的「未精确实测」都保留在 `cost` 原文里。
//!
//! ## 平台专属项
//!
//! `win32-native` / `ui-directwrite` 带 `platforms: ["windows"]`：目标平台不是 Windows
//! 时界面不显示、`features()` 也不会写。数字来自 Windows `x86_64-pc-windows-gnu`、
//! `dist` + strip 实测（见 PrismaRev `docs/DESIGN.md` §12.2 / §12.4）：
//!
//! - `winit` 窗口 3,146,240 B → Win32 原生 2,978,816 B（−163.5 KiB）；
//! - fontdue 文本 3,145,728 B → DirectWrite 3,084,288 B（−60 KiB）。
//!
//! 目录统一按整数 KB 记（`baseline_kb` / `size_delta_kb`），上面两个增量取整成
//! −164 / −60，原文放在 `cost` 里；估算一律标「估算」。
//!
//! ## 体积档位
//!
//! [`CapabilityCatalog::size_tiers`] 描述标准发布 / 极限体积两档。`size_percent` 只是
//! 把引擎已量出的「3D 全量 4.47 MB → 2.70 MB（−40%）」比例套到当前组合上，
//! 界面必须继续标「估算」，并且不能把极限体积档说成竞赛级 64k。

use serde::Serialize;

/// 目录项是哪一类：互斥的维度，还是可多选的能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    Dimension,
    Feature,
}

/// 新建工程向导里的一项能力。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityInfo {
    /// 与 [`crate::projects::ProjectCapabilities::option_state`] 认识的键一致。
    pub id: &'static str,
    pub label: &'static str,
    /// 一句话说明（卡片标题下面那行）。
    pub summary: &'static str,
    pub pros: &'static [&'static str],
    pub cons: &'static [&'static str],
    /// 给人看的成本原文，例如「约 +1.0 MB / +30 crates」；拿不到就写「未实测」。
    pub cost: &'static str,
    /// 向导打开时的默认勾选状态。
    pub default_on: bool,
    /// 这项只影响的平台（`["linux"]` / `["windows"]`）；空 = 全平台。
    pub platforms: &'static [&'static str],
    /// `false` = 界面上置灰并显示 [`Self::disabled_reason`]（引擎侧还没接线）。
    pub enabled: bool,
    pub kind: CapabilityKind,
    /// 基准构建里这项是开还是关；估算只在「状态 ≠ 基准」时累加 `size_delta_kb`。
    pub baseline_on: bool,
    /// 相对基准构建、切到非基准状态时的体积变化（KB，负 = 变小）；`None` = 不参与估算。
    pub size_delta_kb: Option<i64>,
    pub disabled_reason: Option<&'static str>,
}

/// 每个目标平台的基准构建。
///
/// Linux 与 Windows 的发布包不是同一个量级，所以基准按平台分开给——否则 Windows
/// 工程的「预计体积」会拿 Linux 的 4.47 MB 当底。`platform` 与
/// [`crate::projects::TargetPlatform`] 的 serde 名一致。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBaseline {
    pub platform: &'static str,
    /// 基准构建的体积（KB）。
    pub kb: i64,
    /// 基准构建包含什么（界面要显示来源，不能只给一个数字）。
    pub label: &'static str,
    /// 基准是怎么量出来的，以及为什么累加结果是估算。
    pub note: &'static str,
}

/// 体积档位：标准发布 / 极限体积。
///
/// `size_percent` 只是把实测比例套到当前勾选组合上的**估算系数**，不是每个组合的
/// 实测值；界面必须标「估算」，并显示 [`Self::reference`]。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeTierInfo {
    /// 与 [`crate::projects::SizeTier`] 的 serde 名一致（`standard` / `tiny`）。
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub pros: &'static [&'static str],
    pub cons: &'static [&'static str],
    /// 构建命令原文（界面徽章显示这个，不是体积）。
    pub command: &'static str,
    pub default_on: bool,
    /// 相对标准发布的体积百分比（标准 = 100）。只参与估算。
    pub size_percent: u32,
    /// 百分比来自哪次实测。
    pub reference: &'static str,
}

/// 向导用的完整目录：各平台基准 + 每一项能力 + 体积档位。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityCatalog {
    pub baselines: &'static [PlatformBaseline],
    pub entries: &'static [CapabilityInfo],
    pub size_tiers: &'static [SizeTierInfo],
}

const LINUX: &[&str] = &["linux"];
const WINDOWS: &[&str] = &["windows"];

/// 唯一的目录数据源。`size_delta_kb` 的符号与量级和 `cost` 原文必须一致。
pub const CATALOG: CapabilityCatalog = CapabilityCatalog {
    baselines: BASELINES,
    entries: ENTRIES,
    size_tiers: SIZE_TIERS,
};

const BASELINES: &[PlatformBaseline] = &[
    PlatformBaseline {
        platform: "linux",
        kb: 4470,
        label: "3D 全量 + x11 + wayland + ui",
        note: "dist profile、strip、Linux aarch64 实测；其余项按单项实测或依赖体积累加，属估算",
    },
    PlatformBaseline {
        platform: "windows",
        // 3,146,240 B ≈ 3073 KiB（winit 窗口、dist + strip）。
        kb: 3073,
        label: "Windows 3D 全量 + winit 窗口（3,146,240 字节）",
        note: "x86_64-pc-windows-gnu、dist + strip 实测基准；单项增量沿用 Linux 实测，属估算",
    },
];

const SIZE_TIERS: &[SizeTierInfo] = &[
    SizeTierInfo {
        id: "standard",
        label: "标准发布",
        summary: "只生成 `[profile.dist]`（fat LTO + 单 codegen unit + abort + strip）",
        pros: &["stable 工具链直接可用，日常调试与发布同一条命令"],
        cons: &["比极限体积档大约多 40%"],
        command: "cargo build --profile dist",
        default_on: true,
        size_percent: 100,
        reference: "dist profile、strip 实测（3D 全量 4.47 MB）",
    },
    SizeTierInfo {
        id: "tiny",
        label: "极限体积",
        summary:
            "额外生成 `[profile.tiny]`：`opt-level = \"z\"` + fat LTO + 单 codegen unit + abort + strip",
        pros: &[
            "3D 全量 4.47 MB → 2.70 MB（−40%）",
            "配合 nightly `-Z build-std` + immediate-abort 再省 std 与 panic 格式化",
        ],
        cons: &[
            "禁内联后 CPU 热路径慢 10–30%，CPU-bound 场景明显",
            "需要 nightly + rust-src；日常 `cargo run` 仍是 stable",
            "现实区间 2.5–3.2 MB，不是竞赛级 64k",
        ],
        command: "cargo +nightly build --profile tiny",
        default_on: false,
        // 3D 全量 4.47 MB → 2.70 MB ≈ −40%。
        size_percent: 60,
        reference: "aarch64 Linux、3D 全量实测：4.47 MB → 2.70 MB（−40%）",
    },
];

const ENTRIES: &[CapabilityInfo] = &[
    CapabilityInfo {
        id: "3d",
        label: "3D 渲染",
        summary: "网格 / 灯光 / 天空盒的完整 3D 组件组（默认档）",
        pros: &[
            "模板场景自带天空盒 + 透视相机 + 平行光",
            "路径追踪、IBL、阴影都基于这一档",
        ],
        cons: &["2D 游戏会白背整套 3D 组件（2D 档 −197 KB）"],
        cost: "基准档（4.47 MB 基准构建包含这一档）",
        default_on: true,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Dimension,
        baseline_on: true,
        size_delta_kb: Some(0),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "2d",
        label: "2D 渲染",
        summary: "正交相机 + 精灵；不链接 3D 组件",
        pros: &["2D 游戏不背 3D 组件，启动更快"],
        cons: &["没有灯光 / 天空盒 / 网格渲染组件"],
        cost: "−197 KB（相对 3D 档）",
        default_on: false,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Dimension,
        baseline_on: false,
        size_delta_kb: Some(-197),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "ui",
        label: "UI 叠加",
        summary: "文本 / HUD / 调试叠加（`ui` + `group-ui` + 文本后端）",
        pros: &["文本 / HUD / 调试叠加", "模板场景会附带一个示例标签"],
        cons: &[
            "关掉后没有文字渲染，只剩几何",
            "文本后端默认 fontdue；Windows 可换成 DirectWrite（见下一项）",
        ],
        cost: "−64 KB（关掉时）",
        default_on: false,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: true,
        size_delta_kb: Some(-64),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "audio",
        label: "音频",
        summary: "音乐 / 音效播放（symphonia 全套编解码）",
        pros: &["音乐 / 音效，wav / ogg / mp3 / flac"],
        cons: &[
            "体积大头：约 +1.0 MB",
            "当前仓库里没有实际播放代码，属「预留」",
        ],
        cost: "约 +1.0 MB（symphonia 全套编解码）",
        default_on: false,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: false,
        size_delta_kb: Some(1000),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "physics",
        label: "物理",
        summary: "Rapier 刚体 / 碰撞（目前只有骨架）",
        pros: &["后续接刚体 / 碰撞"],
        cons: &[
            "当前只是骨架，未接进 `Engine::update`",
            "约 +2 MB 依赖，未精确实测",
        ],
        cost: "约 +2 MB 依赖（rapier3d / parry3d），未精确实测",
        default_on: false,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: false,
        size_delta_kb: Some(2000),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "pathtrace",
        label: "路径追踪",
        summary: "光追 GI / 阴影 / 反射（`render_mode = path_trace`）",
        pros: &["光追 GI / 阴影 / 反射"],
        cons: &["关掉只有光栅；移动端基本用不上"],
        cost: "−132 KB（关掉时）",
        default_on: false,
        platforms: &[],
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: true,
        size_delta_kb: Some(-132),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "wayland",
        label: "Wayland 窗口",
        summary: "Linux 原生 Wayland 后端（winit）",
        pros: &["原生 Wayland 会话，无 XWayland"],
        cons: &["只影响 Linux；X11-only 构建更小"],
        cost: "约 +1.1 MB（含 tiny-skia / ttf-parser）",
        default_on: true,
        platforms: LINUX,
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: true,
        size_delta_kb: Some(-1100),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "x11",
        label: "X11 窗口",
        summary: "Linux X11 后端（winit）",
        pros: &["兼容所有 Linux 桌面"],
        cons: &["纯 Wayland 环境需 XWayland"],
        cost: "约 +90 KB",
        default_on: true,
        platforms: LINUX,
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: true,
        size_delta_kb: Some(-90),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "win32-native",
        label: "Win32 原生窗口",
        summary: "自带 Win32 窗口后端（仅 Windows），发布包不再链接 winit",
        pros: &[
            "Windows 发布包 −163.5 KiB（3,146,240 → 2,978,816 字节）",
            "依赖树里 winit 计数为 0；消息泵自己实现，不依赖 winit 事件循环",
        ],
        cons: &[
            "与 winit 后端二选一：勾选后不再写 `x11` / `wayland`",
            "没有 IME / 拖放（DnD）/ 多显示器 DPI 平滑，需真机验证",
        ],
        cost: "−163.5 KiB（3,146,240 → 2,978,816 字节，Windows dist + strip）",
        default_on: false,
        platforms: WINDOWS,
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: false,
        // −163.5 KiB，目录统一按整数 KB 记，取整成 −164。
        size_delta_kb: Some(-164),
        disabled_reason: None,
    },
    CapabilityInfo {
        id: "ui-directwrite",
        label: "DirectWrite 文本",
        summary: "用 Windows 系统 DirectWrite 光栅字形，代替 fontdue（仅 Windows）",
        pros: &[
            "Windows 发布包 −60 KiB（3,145,728 → 3,084,288 字节）",
            "用系统字体；fontdue / ttf-parser 不进二进制（勾选时自动带 `ui`）",
        ],
        cons: &[
            "仅 Windows 目标（其它平台开它会编译失败）",
            "只做单字形查表，不支持复杂文本整形（阿拉伯 / 印度语系连写、kerning、Emoji）",
            "真机需确认灰度 AA 清晰度与字体回退",
        ],
        cost: "−60 KiB（3,145,728 → 3,084,288 字节，Windows dist + build-std）",
        default_on: false,
        platforms: WINDOWS,
        enabled: true,
        kind: CapabilityKind::Feature,
        baseline_on: false,
        // 61,440 B = 60 KiB。
        size_delta_kb: Some(-60),
        disabled_reason: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{Dimension, ProjectCapabilities, SizeTier, TargetPlatform};

    /// 目录 id 应当推导出的 feature。和 `ProjectCapabilities::features` 是同一份契约。
    fn expected_features(id: &str) -> &'static [&'static str] {
        match id {
            "3d" => &["group-3d"],
            "2d" => &["group-2d"],
            "ui" => &["ui", "group-ui", "ui-fontdue"],
            "audio" => &["audio"],
            "physics" => &["physics"],
            "pathtrace" => &["pathtrace"],
            "x11" => &["x11"],
            "wayland" => &["wayland"],
            "win32-native" => &["win32-native"],
            "ui-directwrite" => &["ui", "group-ui", "ui-directwrite"],
            _ => &[],
        }
    }

    /// 平台专属项要在它自己的目标平台上才推导得出来。
    fn caps_for(info: &CapabilityInfo) -> ProjectCapabilities {
        let target_platform = if info.platforms.contains(&"windows") {
            TargetPlatform::Windows
        } else {
            TargetPlatform::Linux
        };
        ProjectCapabilities {
            target_platform,
            ..ProjectCapabilities::default()
        }
    }

    fn caps_on(target: TargetPlatform, ids: &[&str]) -> ProjectCapabilities {
        let mut caps = ProjectCapabilities {
            target_platform: target,
            ..ProjectCapabilities::default()
        };
        for id in ids {
            assert!(caps.apply_option(id, true), "{id} 应当已接线");
        }
        caps
    }

    /// 关掉某一项（默认开的窗口后端 / UI 用得上）。
    trait Without {
        fn clone_without(self, id: &str) -> Self;
    }

    impl Without for ProjectCapabilities {
        fn clone_without(mut self, id: &str) -> Self {
            assert!(self.apply_option(id, false), "{id} 应当已接线");
            self
        }
    }

    fn baseline(catalog: &CapabilityCatalog, target: TargetPlatform) -> i64 {
        catalog
            .baselines
            .iter()
            .find(|baseline| baseline.platform == target.as_str())
            .unwrap_or_else(|| panic!("目录缺少 {} 的基准", target.as_str()))
            .kb
    }

    /// 这一项对目标平台是否适用（空 `platforms` = 全平台）。
    fn applies(info: &CapabilityInfo, target: TargetPlatform) -> bool {
        info.platforms.is_empty() || info.platforms.contains(&target.as_str())
    }

    /// 与前端 `estimate` 同一套算法：平台基准 + 偏离基准的项的增量。
    fn estimate(catalog: &CapabilityCatalog, caps: &ProjectCapabilities) -> i64 {
        let target = caps.target_platform;
        catalog
            .entries
            .iter()
            .filter(|info| applies(info, target))
            .fold(baseline(catalog, target), |kb, info| {
                let (Some(delta), Some(state)) = (info.size_delta_kb, caps.option_state(info.id))
                else {
                    return kb;
                };
                if state == info.baseline_on {
                    kb
                } else {
                    kb + delta
                }
            })
    }

    #[test]
    fn catalog_ids_are_unique_and_complete() {
        let mut ids = std::collections::HashSet::new();
        for info in CATALOG.entries {
            assert!(!info.id.is_empty(), "id 不能为空");
            assert!(ids.insert(info.id), "重复的 id：{}", info.id);
            assert!(!info.label.is_empty(), "{} 缺少标题", info.id);
            assert!(!info.summary.is_empty(), "{} 缺少一句话说明", info.id);
            assert!(!info.cost.is_empty(), "{} 缺少成本", info.id);
            assert!(!info.pros.is_empty(), "{} 缺少「利」", info.id);
            assert!(!info.cons.is_empty(), "{} 缺少「弊」", info.id);
            for platform in info.platforms {
                assert!(
                    matches!(*platform, "linux" | "windows"),
                    "{}：未知平台 {platform}",
                    info.id
                );
            }
            if info.enabled {
                assert!(
                    info.size_delta_kb.is_some(),
                    "{} 已接线，必须给出体积增量（或明确 0）",
                    info.id
                );
                assert!(
                    info.disabled_reason.is_none(),
                    "{} 已接线却带着置灰原因",
                    info.id
                );
            } else {
                assert!(
                    info.disabled_reason.is_some(),
                    "{} 置灰必须说明原因，不能只隐藏",
                    info.id
                );
                assert!(
                    info.size_delta_kb.is_none(),
                    "{} 未接线，不该参与体积估算",
                    info.id
                );
            }
        }
        // 维度是单选组：两边都得在，否则界面上会缺一项。
        for id in ["2d", "3d"] {
            assert!(
                CATALOG
                    .entries
                    .iter()
                    .any(|info| info.kind == CapabilityKind::Dimension && info.id == id),
                "维度目录缺少 {id}"
            );
        }
        // Windows 专属的两项已经在引擎侧接线，必须可选（不能再置灰）。
        for id in ["win32-native", "ui-directwrite"] {
            let info = CATALOG
                .entries
                .iter()
                .find(|info| info.id == id)
                .unwrap_or_else(|| panic!("目录缺少 {id}"));
            assert!(info.enabled, "{id} 已接线，应当可选");
            assert_eq!(info.platforms, WINDOWS, "{id} 只影响 Windows");
        }
    }

    #[test]
    fn every_catalog_id_is_recognised_by_the_derivation() {
        for info in CATALOG.entries {
            let mut caps = caps_for(info);
            let applied = caps.apply_option(info.id, true);
            assert_eq!(
                applied, info.enabled,
                "{}：`apply_option` 的接线状态与目录 enabled 不一致",
                info.id
            );
            if !info.enabled {
                assert_eq!(caps, caps_for(info), "{} 未接线，却改了能力状态", info.id);
                continue;
            }
            assert_eq!(
                caps.option_state(info.id),
                Some(true),
                "{}：应用之后状态没有变成开",
                info.id
            );
            let features = caps.features();
            for expected in expected_features(info.id) {
                assert!(
                    features.iter().any(|feature| feature == expected),
                    "{}：features 里缺少 {expected}，实际是 {features:?}",
                    info.id
                );
            }
        }
    }

    #[test]
    fn catalog_defaults_match_the_form_defaults() {
        let default = ProjectCapabilities::default();
        for info in CATALOG.entries {
            let expected = info.enabled.then_some(info.default_on);
            assert_eq!(
                default.option_state(info.id),
                expected,
                "{}：目录 default_on 与 ProjectCapabilities::default() 不一致",
                info.id
            );
        }
    }

    #[test]
    fn disabled_options_never_reach_the_feature_list() {
        let mut caps = caps_with(&["ui", "audio", "physics", "pathtrace", "x11", "wayland"]);
        let before = caps.features();
        for info in CATALOG.entries.iter().filter(|info| !info.enabled) {
            assert!(
                !caps.apply_option(info.id, true),
                "{} 未接线，`apply_option` 不该报告成功",
                info.id
            );
            assert_eq!(caps.option_state(info.id), None, "{} 不该有状态", info.id);
            assert!(
                !caps.features().iter().any(|feature| feature == info.id),
                "{} 未接线，不能出现在 feature 列表里",
                info.id
            );
        }
        assert_eq!(caps.features(), before, "置灰项不能改变任何 feature");
    }

    fn caps_with(ids: &[&str]) -> ProjectCapabilities {
        caps_on(TargetPlatform::Linux, ids)
    }

    #[test]
    fn dimension_options_are_mutually_exclusive() {
        let mut caps = caps_with(&["2d"]);
        assert_eq!(caps.dimension, Dimension::TwoD);
        assert_eq!(caps.option_state("3d"), Some(false));
        assert!(caps.apply_option("3d", true));
        assert_eq!(caps.dimension, Dimension::ThreeD);
        assert_eq!(caps.option_state("2d"), Some(false));
        // 单选组里「关掉 2D」等于切回 3D，不会两边都关。
        assert!(caps.apply_option("2d", true));
        assert!(caps.apply_option("2d", false));
        assert_eq!(caps.dimension, Dimension::ThreeD);
    }

    #[test]
    fn windows_options_replace_the_linux_backends() {
        // 选了 win32-native 就不再写 x11 / wayland，也不写 winit-backend。
        let caps = caps_on(
            TargetPlatform::Windows,
            &["3d", "ui", "win32-native", "ui-directwrite"],
        );
        let features = caps.features();
        for feature in ["win32-native", "ui-directwrite", "ui-fontdue"] {
            let expected = !matches!(feature, "ui-fontdue");
            assert_eq!(
                features.iter().any(|item| item == feature),
                expected,
                "{feature}：{features:?}"
            );
        }
        for feature in ["x11", "wayland", "winit-backend"] {
            assert!(
                !features.iter().any(|item| item == feature),
                "选了 win32-native 不该再写 {feature}：{features:?}"
            );
        }

        // 不选 win32-native 时仍然是 winit（Windows 上的默认后端）。
        let caps = caps_on(TargetPlatform::Windows, &["3d"]);
        assert_eq!(caps.features(), ["group-3d", "winit-backend"]);
    }

    #[test]
    fn text_backends_are_mutually_exclusive() {
        let cases = [
            (TargetPlatform::Linux, false),
            (TargetPlatform::Windows, false),
            (TargetPlatform::Windows, true),
        ];
        for (target, directwrite) in cases {
            let mut caps = caps_on(target, &["ui"]);
            caps.ui_directwrite = directwrite;
            let features = caps.features();
            let fontdue = features.iter().filter(|item| *item == "ui-fontdue").count();
            let dw = features
                .iter()
                .filter(|item| *item == "ui-directwrite")
                .count();
            assert_eq!(fontdue + dw, 1, "{target:?}: {features:?}");
            assert_eq!(dw, usize::from(directwrite), "{target:?}: {features:?}");
        }
    }

    #[test]
    fn ui_directwrite_implies_ui() {
        let mut caps = caps_on(TargetPlatform::Windows, &[]);
        assert!(caps.apply_option("ui-directwrite", true));
        assert!(caps.ui);
        assert_eq!(caps.option_state("ui"), Some(true));
        let features = caps.features();
        assert!(features.contains(&"ui".to_string()), "{features:?}");
        assert!(features.contains(&"group-ui".to_string()), "{features:?}");
    }

    #[test]
    fn platform_specific_options_never_leak_to_the_other_platform() {
        let mut caps = caps_on(TargetPlatform::Linux, &["3d"]);
        caps.win32_native = true;
        caps.ui_directwrite = true;
        let features = caps.features();
        assert!(
            !features
                .iter()
                .any(|item| item == "win32-native" || item == "ui-directwrite"),
            "{features:?}"
        );
        // `ui-directwrite` 隐含 `ui`，但在 Linux 上会被换成 fontdue；这种组合本身
        // 会被 `validate()` 拒绝，`features()` 只是不写平台专属项。
        assert!(
            features.iter().any(|item| item == "ui-fontdue"),
            "{features:?}"
        );

        let mut caps = caps_on(TargetPlatform::Windows, &["3d"]);
        caps.x11 = true;
        caps.wayland = true;
        let features = caps.features();
        assert!(
            !features
                .iter()
                .any(|item| item == "x11" || item == "wayland"),
            "{features:?}"
        );
    }

    #[test]
    fn windows_only_options_are_rejected_on_linux() {
        let caps = ProjectCapabilities {
            win32_native: true,
            ..ProjectCapabilities::default()
        };
        assert!(caps.validate().is_err());
        let caps = ProjectCapabilities {
            ui_directwrite: true,
            ..ProjectCapabilities::default()
        };
        assert!(caps.validate().is_err());
        // Windows 目标不要求 x11 / wayland。
        let caps = ProjectCapabilities {
            target_platform: TargetPlatform::Windows,
            x11: false,
            wayland: false,
            ..ProjectCapabilities::default()
        };
        assert!(caps.validate().is_ok());
    }

    #[test]
    fn baseline_estimate_reproduces_the_reference_build() {
        // 基准 = 3D 全量 + x11 + wayland + ui（= 4.47 MB，`pathtrace` 也在基准里：
        // 表里的成本是「关掉时 −132 KB」量出来的）。
        let caps = caps_with(&["3d", "ui", "pathtrace", "x11", "wayland"]);
        assert_eq!(
            estimate(&CATALOG, &caps),
            baseline(&CATALOG, TargetPlatform::Linux)
        );
        assert_eq!(baseline(&CATALOG, TargetPlatform::Linux), 4470);
    }

    #[test]
    fn estimates_follow_the_measured_deltas() {
        // 三个验收工程：2D+UI / 3D+UI+音频 / 2D 极简。
        assert_eq!(
            estimate(&CATALOG, &caps_with(&["2d", "ui"])),
            4470 - 197 - 132
        );
        assert_eq!(
            estimate(&CATALOG, &caps_with(&["3d", "ui", "audio"])),
            4470 + 1000 - 132
        );
        assert_eq!(
            estimate(&CATALOG, &caps_with(&["2d"])),
            4470 - 197 - 64 - 132
        );
        // 关掉 Wayland（X11-only）省下的就是它的重量。
        let x11_only = caps_with(&["x11"]).clone_without("wayland");
        assert_eq!(estimate(&CATALOG, &x11_only), 4470 - 64 - 132 - 1100);
    }

    #[test]
    fn windows_estimates_use_the_windows_baseline() {
        // Windows 基准 = 3D 全量 + winit（3,146,240 B ≈ 3073 KiB）。
        let caps = {
            let mut caps = caps_on(TargetPlatform::Windows, &["3d", "ui", "pathtrace"]);
            caps.x11 = true;
            caps.wayland = true;
            caps
        };
        assert_eq!(estimate(&CATALOG, &caps), 3073);
        // 打开 Win32 原生窗口：−163.5 KiB；再换 DirectWrite：−60 KiB。
        let win32 = ProjectCapabilities {
            win32_native: true,
            ..caps.clone()
        };
        assert_eq!(estimate(&CATALOG, &win32), 3073 - 164);
        let directwrite = ProjectCapabilities {
            ui_directwrite: true,
            ..win32
        };
        assert_eq!(estimate(&CATALOG, &directwrite), 3073 - 164 - 60);
        // Linux 后端在 Windows 估算里不参与（基准里它们本来就是开的）。
        let no_linux = ProjectCapabilities {
            x11: false,
            wayland: false,
            ..caps
        };
        assert_eq!(estimate(&CATALOG, &no_linux), 3073);
    }

    #[test]
    fn size_tiers_cover_standard_and_tiny() {
        let ids: Vec<&str> = CATALOG.size_tiers.iter().map(|tier| tier.id).collect();
        assert_eq!(ids, ["standard", "tiny"]);
        for tier in CATALOG.size_tiers {
            assert!(!tier.label.is_empty(), "{} 缺少标题", tier.id);
            assert!(!tier.summary.is_empty(), "{} 缺少说明", tier.id);
            assert!(!tier.command.is_empty(), "{} 缺少构建命令", tier.id);
            assert!(!tier.pros.is_empty(), "{} 缺少「利」", tier.id);
            assert!(!tier.cons.is_empty(), "{} 缺少「弊」", tier.id);
            assert!(!tier.reference.is_empty(), "{} 缺少实测出处", tier.id);
            // id 必须能被 SizeTier 的 serde 名识别。
            assert!(
                matches!(tier.id, "standard" | "tiny"),
                "未知档位 {}",
                tier.id
            );
        }
        let standard = &CATALOG.size_tiers[0];
        assert!(standard.default_on);
        assert_eq!(standard.size_percent, 100);
        assert_eq!(SizeTier::default().as_str(), standard.id);

        let tiny = &CATALOG.size_tiers[1];
        assert!(!tiny.default_on);
        assert_eq!(tiny.size_percent, 60, "3D 全量 4.47 MB → 2.70 MB ≈ −40%");
        // 代价必须写清楚：慢 10–30%，且不是竞赛级 64k。
        assert!(
            tiny.cons.iter().any(|con| con.contains("10–30%")),
            "{tiny:?}"
        );
        assert!(tiny.cons.iter().any(|con| con.contains("64k")), "{tiny:?}");
        assert!(
            tiny.cons.iter().any(|con| con.contains("2.5–3.2 MB")),
            "{tiny:?}"
        );
    }

    /// 引擎侧实测表：`(id, size_delta_kb, baseline_on)`。改数字必须改这张表。
    #[test]
    fn size_deltas_match_the_measured_table() {
        let expected: &[(&str, Option<i64>, bool)] = &[
            ("3d", Some(0), true),
            ("2d", Some(-197), false),
            ("ui", Some(-64), true),
            ("audio", Some(1000), false),
            ("physics", Some(2000), false),
            ("pathtrace", Some(-132), true),
            ("wayland", Some(-1100), true),
            ("x11", Some(-90), true),
            ("win32-native", Some(-164), false),
            ("ui-directwrite", Some(-60), false),
        ];
        assert_eq!(
            CATALOG.entries.len(),
            expected.len(),
            "目录项数量变了，同步更新这张实测表"
        );
        for (id, delta, baseline_on) in expected {
            let info = CATALOG
                .entries
                .iter()
                .find(|info| info.id == *id)
                .unwrap_or_else(|| panic!("目录缺少 {id}"));
            assert_eq!(info.size_delta_kb, *delta, "{id} 的 size_delta_kb");
            assert_eq!(info.baseline_on, *baseline_on, "{id} 的 baseline_on");
            // 表里用「关掉」计量的项，基准里一定得是开的。
            if info.cost.contains("关掉") {
                assert!(info.baseline_on, "{id}：以「关掉」计量却在基准里是关的");
                assert!(info.size_delta_kb.unwrap_or(0) < 0, "{id}：关掉应当省体积");
            }
        }
    }
}
