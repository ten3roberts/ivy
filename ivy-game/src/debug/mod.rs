use std::time::{Duration, Instant};

use glam::BVec2;
use itertools::Itertools;
use ivy_assets::{timeline::Timelines, AssetCache};
use violet::{
    core::{
        components::offset,
        layout::Align,
        style::{
            base_colors::PLATINUM_700, default_corner_radius, spacing_medium, spacing_small,
            SizeExt,
        },
        text::Wrap,
        unit::Unit,
        widget::{
            card, col, interactive::tooltip::Tooltip, label, pill, Float, Rectangle, ScrollArea,
            SignalWidget, Stack,
        },
        Scope, Widget,
    },
    futures_signals::signal::ReadOnlyMutable,
    palette::{Hsla, IntoColor, Srgba},
};

pub struct AssetTimelinesWidget {
    assets: AssetCache,
}

impl AssetTimelinesWidget {
    pub fn new(assets: AssetCache) -> Self {
        Self { assets }
    }
}

impl Widget for AssetTimelinesWidget {
    fn mount(self, scope: &mut violet::core::Scope<'_>) {
        let timelines = TimelinesWidget {
            timelines: self.assets.timelines(),
        };

        card(ScrollArea::new(BVec2::TRUE, timelines))
            .with_max_size(Unit::px2(1200.0, 800.0))
            .mount(scope);
    }
}

pub struct TimelinesWidget {
    timelines: ReadOnlyMutable<Timelines>,
}

impl Widget for TimelinesWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        let values = self.timelines.signal_ref(|timelines| {
            let begin_load = timelines
                .spans()
                .iter()
                .next()
                .map(|v| v.1.load_start())
                .unwrap_or(Instant::now());

            let now = Instant::now();

            let end_load = timelines
                .spans()
                .iter()
                .last()
                .map(|v| v.1.load_end().unwrap_or(now))
                .unwrap_or(Instant::now());

            let mut args = Args {
                rows: Vec::new(),
                result: Vec::new(),
            };

            for &id in timelines.roots() {
                iter_timelines(timelines, id, begin_load, now, &mut args, id);
            }

            Stack::new((
                Ruler {
                    start: Duration::ZERO,
                    end: end_load.duration_since(begin_load),
                },
                Stack::new(args.result),
            ))
        });

        SignalWidget::new(values).mount(scope);
    }
}

struct Row {
    max_column: f32,
}

struct Args {
    rows: Vec<Row>,
    result: Vec<TimespanWidget>,
}

fn iter_timelines(
    timelines: &Timelines,
    id: usize,
    begin: Instant,
    now: Instant,
    args: &mut Args,
    root: usize,
) {
    let span = &timelines.spans()[id];

    let load_end = span.load_end().unwrap_or(now);

    let end = load_end.duration_since(begin).as_secs_f32();
    let duration = load_end.duration_since(span.load_start());

    let start = span.load_start().duration_since(begin).as_secs_f32();
    let padded_end = end.max(start + 0.1);

    let row = if let Some((row_index, found_row)) = args
        .rows
        .iter_mut()
        .enumerate()
        .rev()
        .take_while(|(_, row)| row.max_column < start)
        .last()
    {
        found_row.max_column = padded_end;
        row_index
    } else {
        // if args.rows.last().is_some_and(|v| v.root != root) {
        //     args.rows.push(Row {
        //         max_column: end,
        //         root,
        //     });
        // }

        args.rows.push(Row {
            max_column: padded_end,
        });
        args.rows.len() - 1
    };

    let span_name = &span.info().name;

    let widget = TimespanWidget {
        start,
        end,
        padded_end,
        row,
        text: format!("{duration:.0?} {span_name}",),
        color: Hsla::new(root as f32 * 15.0, 0.5, 0.4, 1.0).into_color(),
    };

    args.result.push(widget);

    for &child in timelines.edge_map().get(&id).into_iter().flatten() {
        iter_timelines(timelines, child, begin, now, args, root);
    }
}

struct Ruler {
    start: Duration,
    end: Duration,
}

impl Widget for Ruler {
    fn mount(self, scope: &mut Scope<'_>) {
        // create tickmarks for each second
        let start_ms = (self.start.as_millis() / 100) as u32 * 100;
        let end_ms = self.end.as_millis().div_ceil(100) as u32 * 100;

        let tickmarks = (start_ms..=end_ms)
            .step_by(100)
            .map(|i| TickMark {
                time: Duration::from_millis(i as u64),
            })
            .collect_vec();

        Stack::new(tickmarks)
            .with_background(Srgba::new(0.0, 0.0, 0.0, 0.2))
            .with_padding(spacing_medium())
            .mount(scope);
    }
}

struct TickMark {
    time: Duration,
}

impl Widget for TickMark {
    fn mount(self, scope: &mut Scope<'_>) {
        let start = self.time.as_secs_f32() * TIMELINE_SCALE;

        scope.set(offset(), Unit::px2(start, 0.0));

        Stack::new(
            col((
                label(format!("{:?}", self.time)).with_wrap(Wrap::None),
                Rectangle::new(PLATINUM_700).with_exact_size(Unit::px2(2.0, 8.0)),
            ))
            .with_cross_align(Align::Center),
        )
        .mount(scope);
    }
}

struct TimespanWidget {
    start: f32,
    end: f32,
    row: usize,
    text: String,
    color: Srgba,
    padded_end: f32,
}

pub const TIMELINE_SCALE: f32 = 2400.0;
pub const CELL_HEIGHT: f32 = 26.0;
pub const CELL_SPACING: f32 = 4.0;

impl Widget for TimespanWidget {
    fn mount(self, scope: &mut violet::core::Scope<'_>) {
        let start = self.start * TIMELINE_SCALE;
        let end = self.end * TIMELINE_SCALE;

        scope.set(
            offset(),
            Unit::px2(
                start + CELL_SPACING,
                (self.row + 2) as f32 * (CELL_HEIGHT + CELL_SPACING),
            ),
        );

        let width = (end - start - CELL_SPACING * 2.0).max(2.0);
        let padded_width = (self.padded_end * TIMELINE_SCALE - start - CELL_SPACING * 2.0).max(2.0);

        Tooltip::new(
            Stack::new((
                Rectangle::new(self.color)
                    .with_exact_size(Unit::px2(width, CELL_HEIGHT))
                    .with_corner_radius(default_corner_radius()),
                Stack::new(label(&self.text).with_wrap(Wrap::None))
                    .with_clip(BVec2::new(true, true))
                    .with_max_size(Unit::px2(padded_width, CELL_HEIGHT)),
            ))
            .with_size(Unit::px2(padded_width, CELL_HEIGHT + CELL_SPACING))
            .with_padding(spacing_small())
            .with_alignment(Align::Start, Align::Center),
            move || pill(label(&self.text)),
        )
        .mount(scope);
    }
}
