use super::images::Images;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2, vec2};
use std::ops::Range;

pub const MIN_CARD_WIDTH: f32 = 110.0;
pub const MAX_CARD_WIDTH: f32 = 340.0;
const CARD_RATIO: f32 = 680.0 / 488.0;
const LABEL_HEIGHT: f32 = 20.0;
const CORNER: f32 = 8.0;

pub const OWNED_COLOR: Color32 = Color32::from_rgb(46, 125, 50);
pub const MISSING_COLOR: Color32 = Color32::from_rgb(183, 28, 28);
pub const NEUTRAL_COLOR: Color32 = Color32::from_rgb(55, 71, 79);

pub struct Tile<'a> {
    pub name: &'a str,
    pub image_url: Option<&'a str>,
    pub badge: Option<Badge>,
    pub dimmed: bool,
}

pub struct Badge {
    pub text: String,
    pub color: Color32,
}

struct Layout {
    columns: usize,
    size: Vec2,
}

pub fn scrolling(
    ui: &mut egui::Ui,
    id_salt: &str,
    tiles: &[Tile],
    card_width: f32,
    images: &mut Images,
) -> Option<usize> {
    let layout = layout(ui, card_width);
    let rows = tiles.len().div_ceil(layout.columns);
    let mut clicked = None;

    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink(false)
        .show_rows(ui, layout.size.y, rows, |ui, visible| {
            clicked = paint_rows(ui, tiles, &layout, visible, images);
        });
    clicked
}

pub fn inline(
    ui: &mut egui::Ui,
    tiles: &[Tile],
    card_width: f32,
    images: &mut Images,
) -> Option<usize> {
    let layout = layout(ui, card_width);
    let rows = tiles.len().div_ceil(layout.columns);
    paint_rows(ui, tiles, &layout, 0..rows, images)
}

fn layout(ui: &egui::Ui, card_width: f32) -> Layout {
    let gap = ui.spacing().item_spacing.x;
    let available = ui.available_width() - 4.0;
    let columns = (((available + gap) / (card_width + gap)).floor() as usize).max(1);
    let width = ((available - gap * (columns - 1) as f32) / columns as f32).floor();

    Layout {
        columns,
        size: vec2(width, (width * CARD_RATIO).round() + LABEL_HEIGHT),
    }
}

fn paint_rows(
    ui: &mut egui::Ui,
    tiles: &[Tile],
    layout: &Layout,
    rows: Range<usize>,
    images: &mut Images,
) -> Option<usize> {
    let mut clicked = None;

    for row in rows {
        let start = row * layout.columns;
        let end = (start + layout.columns).min(tiles.len());
        ui.horizontal(|ui| {
            for (offset, tile) in tiles[start..end].iter().enumerate() {
                if paint_tile(ui, tile, layout.size, images).clicked() {
                    clicked = Some(start + offset);
                }
            }
        });
    }
    clicked
}

fn paint_tile(ui: &mut egui::Ui, tile: &Tile, size: Vec2, images: &mut Images) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let image_rect = Rect::from_min_size(rect.min, vec2(size.x, size.y - LABEL_HEIGHT));
    match tile.image_url.and_then(|url| images.uri(url)) {
        Some(uri) => egui::Image::new(uri)
            .corner_radius(CORNER)
            .paint_at(ui, image_rect),
        None => paint_placeholder(ui, image_rect, tile.name),
    }

    let painter = ui.painter();
    if tile.dimmed {
        painter.rect_filled(image_rect, CORNER, Color32::from_black_alpha(160));
    }
    if response.hovered() {
        let stroke = Stroke::new(2.0, ui.visuals().selection.stroke.color);
        painter.rect_stroke(image_rect, CORNER, stroke, StrokeKind::Outside);
    }
    if let Some(badge) = &tile.badge {
        paint_badge(ui, image_rect, badge);
    }

    let label_rect = Rect::from_min_size(image_rect.left_bottom(), vec2(size.x, LABEL_HEIGHT));
    painter.with_clip_rect(label_rect).text(
        label_rect.left_center() + vec2(2.0, 0.0),
        Align2::LEFT_CENTER,
        tile.name,
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    response.on_hover_text(tile.name)
}

fn paint_placeholder(ui: &egui::Ui, rect: Rect, name: &str) {
    let painter = ui.painter();
    painter.rect_filled(rect, CORNER, ui.visuals().faint_bg_color);

    let galley = painter.layout(
        name.to_string(),
        FontId::proportional(13.0),
        ui.visuals().weak_text_color(),
        rect.width() - 16.0,
    );
    let position = rect.center() - galley.size() / 2.0;
    painter.galley(position, galley, ui.visuals().weak_text_color());
}

fn paint_badge(ui: &egui::Ui, image_rect: Rect, badge: &Badge) {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(
        badge.text.clone(),
        FontId::proportional(13.0),
        Color32::WHITE,
    );

    let size = galley.size() + vec2(12.0, 6.0);
    let top_right = image_rect.right_top() + vec2(-8.0, 8.0);
    let rect = Rect::from_min_size(top_right - vec2(size.x, 0.0), size);

    painter.rect_filled(rect, 6.0, badge.color);
    painter.galley(rect.min + vec2(6.0, 3.0), galley, Color32::WHITE);
}
