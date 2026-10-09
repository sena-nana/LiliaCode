//! Chart options for the desktop's three charts: the composer's context ring,
//! share-of-total rings (quota and surfaces) and the daily token trend.

use std::sync::Arc;

use lilia_feature_usage::QuotaUsageDailyBucket;
use nana_ui::runtime::chart::{
    Animation, AreaStyle, Axis, ChartColor, ChartOption, LineSeries, PieItem, PieLabelPosition,
    PieSeries, Tooltip, TooltipFormatter,
};
use nana_ui::runtime::{Chart, LengthSpec, NodeStyle, SemanticColorRole};

/// Slice colours of a share-of-total ring, in slice order.
pub(crate) const SHARE_COLORS: [SemanticColorRole; 5] = [
    SemanticColorRole::Accent,
    SemanticColorRole::Success,
    SemanticColorRole::Warning,
    SemanticColorRole::Text,
    SemanticColorRole::Muted,
];

fn square(style: &mut NodeStyle, size: f32) {
    let layout = Arc::make_mut(&mut style.layout);
    let edge = LengthSpec::Px(size);
    layout.width = Some(edge);
    layout.height = Some(edge);
    layout.min_width = Some(edge);
    layout.min_height = Some(edge);
    layout.flex_grow = Some(0.0);
    layout.flex_shrink = Some(0.0);
}

/// The composer's context-window usage: a small ring with no interaction.
pub(crate) fn usage_ring(percent: i32) -> Chart {
    let used = f64::from(percent.clamp(0, 100));
    let option = ChartOption::new()
        .tooltip(Tooltip::hidden())
        .animation(Animation::disabled())
        .series(
            PieSeries::new(
                "上下文",
                [
                    PieItem::new("已用", used).color(ChartColor::Role(SemanticColorRole::Accent)),
                    PieItem::new("剩余", (100.0 - used).max(0.0))
                        .color(ChartColor::Role(SemanticColorRole::BorderSoft)),
                ],
            )
            .ring(0.62, 1.0)
            .pad(0.0)
            .label(PieLabelPosition::None),
        );
    let mut chart = Chart::new(option).label(format!("上下文 {used:.0}%"));
    square(&mut chart.style, 22.0);
    Arc::make_mut(&mut chart.style.layout).pointer_events =
        Some(nana_ui_core::PointerEventsSpec::None);
    chart
}

/// How `slices` split their total. Hovering a slice names it with its value
/// and share; the accessible name lists every slice.
pub(crate) fn share_ring(name: &str, slices: &[(String, f64)], size: f32) -> Chart {
    let total: f64 = slices.iter().map(|(_, value)| *value).sum();
    let share = move |value: f64| {
        if total > 0.0 {
            value / total * 100.0
        } else {
            0.0
        }
    };
    let names: Arc<[Arc<str>]> = slices
        .iter()
        .map(|(name, _)| Arc::from(name.as_str()))
        .collect();
    let option = ChartOption::new()
        .tooltip(
            Tooltip::default().formatter(TooltipFormatter::new(move |items| {
                let Some(item) = items.first() else {
                    return (Arc::from(""), Vec::new());
                };
                (
                    names.get(item.data_index).cloned().unwrap_or_default(),
                    vec![Arc::from(format!(
                        "{:.0} ({:.1}%)",
                        item.value,
                        share(item.value)
                    ))],
                )
            })),
        )
        .series(
            PieSeries::new(
                name,
                slices
                    .iter()
                    .zip(SHARE_COLORS)
                    .map(|((name, value), color)| {
                        PieItem::new(name.as_str(), *value).color(ChartColor::Role(color))
                    }),
            )
            .ring(0.58, 0.95)
            .pad(1.0)
            .label(PieLabelPosition::None),
        );
    let summary = slices
        .iter()
        .map(|(name, value)| format!("{name}: {value:.0} ({:.1}%)", share(*value)))
        .collect::<Vec<_>>()
        .join("；");
    let mut chart = Chart::new(option).label(summary);
    square(&mut chart.style, size);
    chart
}

/// Daily token use, stacked by kind. The tooltip gives the day's total,
/// each kind, the known cost and the record count.
pub(crate) fn token_trend(daily: &[QuotaUsageDailyBucket], height: f32) -> Chart {
    let days: Arc<[Arc<str>]> = daily
        .iter()
        .map(|bucket| {
            let date = crate::desktop::format_civil_date(bucket.day_start);
            Arc::from(date.get(date.len().saturating_sub(5)..).unwrap_or(&date))
        })
        .collect();
    let buckets: Arc<[QuotaUsageDailyBucket]> = daily.into();
    let values = |pick: fn(&QuotaUsageDailyBucket) -> i64| {
        daily
            .iter()
            .map(|bucket| pick(bucket).max(0) as f64)
            .collect::<Vec<_>>()
    };
    let layer = |name: &'static str, pick: fn(&QuotaUsageDailyBucket) -> i64, color| {
        LineSeries::new(name, values(pick))
            .stack("tokens")
            .area(AreaStyle::default())
            .color(ChartColor::Role(color))
    };
    let tooltip_days = Arc::clone(&days);
    let tooltip = TooltipFormatter::new(move |items| {
        let Some(bucket) = items.first().and_then(|item| buckets.get(item.data_index)) else {
            return (Arc::from(""), Vec::new());
        };
        let day = items
            .first()
            .and_then(|item| tooltip_days.get(item.data_index))
            .cloned()
            .unwrap_or_default();
        let rows = [
            format!("总量: {}", bucket.total_tokens),
            format!("输入: {}", bucket.input_tokens),
            format!("输出: {}", bucket.output_tokens),
            format!("缓存命中: {}", bucket.cache_read_tokens),
            format!("缓存写入: {}", bucket.cache_creation_tokens),
            format!(
                "成本 {}",
                bucket
                    .known_cost_usd
                    .map(|cost| format!("${cost:.4}"))
                    .unwrap_or_else(|| "--".into())
            ),
            format!("记录 {}", bucket.record_count),
        ];
        (day, rows.into_iter().map(Arc::from).collect())
    });
    let option = ChartOption::new()
        .tooltip(Tooltip::default().formatter(tooltip))
        .x_axis(Axis::category(days.iter().cloned()))
        .y_axis(Axis::value())
        .series(layer(
            "输入",
            |bucket| bucket.input_tokens,
            SemanticColorRole::Accent,
        ))
        .series(layer(
            "输出",
            |bucket| bucket.output_tokens,
            SemanticColorRole::Success,
        ))
        .series(layer(
            "缓存命中",
            |bucket| bucket.cache_read_tokens,
            SemanticColorRole::Warning,
        ))
        .series(layer(
            "缓存写入",
            |bucket| bucket.cache_creation_tokens,
            SemanticColorRole::Muted,
        ));
    let mut chart = Chart::new(option).label("每日用量");
    Arc::make_mut(&mut chart.style.layout).height = Some(LengthSpec::Px(height));
    chart
}

/// What a hovered chart shows, for Agent Debug: the datum's index, value and
/// label. Category charts sum every series at the hovered category.
pub(crate) fn hovered_datum(chart: &Chart) -> Option<(usize, f64, String)> {
    use nana_ui::runtime::chart::{hit::NO_HOVER, SeriesData};
    let [series, index] = chart.hover_state().current;
    if index == NO_HOVER {
        return None;
    }
    let index = index as usize;
    let option = &chart.option;
    if let Some(nana_ui::runtime::chart::Series::Pie(pie)) = option.series.get(series as usize) {
        let item = pie.data.get(index)?;
        return Some((index, item.value, item.name.to_string()));
    }
    let value = option
        .series
        .iter()
        .filter_map(|series| match series {
            nana_ui::runtime::chart::Series::Line(line) => Some(&line.data),
            nana_ui::runtime::chart::Series::Bar(bar) => Some(&bar.data),
            _ => None,
        })
        .filter_map(|data| match data {
            SeriesData::Values(values) => values.get(index).copied(),
            SeriesData::Points(points) => points.get(index).map(|point| point[1]),
        })
        .filter(|value| value.is_finite())
        .sum();
    let label = option
        .x_axis
        .first()
        .and_then(|axis| axis.data.get(index))
        .map(|label| label.to_string())
        .unwrap_or_default();
    Some((index, value, label))
}
