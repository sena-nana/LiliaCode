use lilia_feature_usage::QuotaUsageDailyBucket;
use nana_ui::runtime::{
    LengthSpec, NodeStyle, SemanticColorRole, TimeSeriesChart, TimeSeriesLayer,
};
use std::sync::Arc;

pub(super) fn toolbar(window_width: f32) -> nana_ui::runtime::Stack {
    nana_ui::runtime::Stack::bar(8.0)
        .align(if window_width <= 860.0 {
            nana_ui::runtime::AlignSpec::Start
        } else {
            nana_ui::runtime::AlignSpec::Center
        })
        .wrap(true)
}

pub(super) fn action_button(id: &str, label: &str) -> nana_ui::runtime::Button {
    let (label, width) = match id {
        "cycle-quota-days" => (
            format!(
                "近 {}",
                if label.trim().is_empty() {
                    "30 天"
                } else {
                    label
                }
            ),
            104.0,
        ),
        "cycle-quota-backend" => {
            let label = match label {
                "" | "all" => "全部后端",
                "native" => "Native",
                _ => label,
            };
            (format!("范围：{label}"), 168.0)
        }
        _ => (label.to_owned(), 104.0),
    };
    let mut button = super::product_action_button(&label, false);
    let layout = Arc::make_mut(&mut button.style.layout);
    layout.min_width = Some(LengthSpec::Px(width));
    layout.flex_shrink = Some(0.0);
    button
}

pub(super) fn trend(daily: &[QuotaUsageDailyBucket], window_width: f32) -> TimeSeriesChart {
    let values = |pick: fn(&QuotaUsageDailyBucket) -> i64| {
        daily.iter().map(move |bucket| pick(bucket).max(0) as f64)
    };
    let mut style = NodeStyle::default();
    Arc::make_mut(&mut style.layout).height = Some(LengthSpec::Px(if window_width <= 860.0 {
        220.0
    } else {
        248.0
    }));
    TimeSeriesChart::new(values(|bucket| bucket.total_tokens))
        .label("总量")
        .axis_labels(daily.iter().map(|bucket| {
            let date = crate::desktop::format_civil_date(bucket.day_start);
            date.get(date.len().saturating_sub(5)..)
                .unwrap_or(&date)
                .to_owned()
        }))
        .tooltip_details(daily.iter().map(|bucket| {
            format!(
                "成本 {}\n记录 {}",
                bucket
                    .known_cost_usd
                    .map(|cost| format!("${cost:.4}"))
                    .unwrap_or_else(|| "--".into()),
                bucket.record_count
            )
        }))
        .stacked([
            TimeSeriesLayer::new(
                "输入",
                values(|bucket| bucket.input_tokens),
                SemanticColorRole::Accent,
            ),
            TimeSeriesLayer::new(
                "输出",
                values(|bucket| bucket.output_tokens),
                SemanticColorRole::Success,
            ),
            TimeSeriesLayer::new(
                "缓存命中",
                values(|bucket| bucket.cache_read_tokens),
                SemanticColorRole::Warning,
            ),
            TimeSeriesLayer::new(
                "缓存写入",
                values(|bucket| bucket.cache_creation_tokens),
                SemanticColorRole::Muted,
            ),
        ])
        .style(style)
}
