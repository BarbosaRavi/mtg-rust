use super::images::Images;
use eframe::egui;
use rust_mtg::card_store::CardRecord;
use rust_mtg::catalog::CardView;

const IMAGE_WIDTH: f32 = 300.0;

pub struct CardDetail {
    pub name: String,
    pub record: Option<CardRecord>,
}

impl From<&CardView> for CardDetail {
    fn from(card: &CardView) -> Self {
        Self {
            name: card.name.clone(),
            record: card.details.clone(),
        }
    }
}

pub fn show(ctx: &egui::Context, detail: &mut Option<CardDetail>, images: &mut Images) {
    let Some(card) = detail.as_ref() else {
        return;
    };
    let mut open = true;

    egui::Window::new(card.name.as_str())
        .id(egui::Id::new("card_detail"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| contents(ui, card, images));

    if !open {
        *detail = None;
    }
}

fn contents(ui: &mut egui::Ui, card: &CardDetail, images: &mut Images) {
    let Some(record) = &card.record else {
        ui.label("Sem dados da Scryfall para esta carta.");
        return;
    };

    ui.horizontal_top(|ui| {
        if let Some(url) = &record.image_normal {
            match images.uri(url) {
                Some(uri) => {
                    ui.add(
                        egui::Image::new(uri)
                            .max_width(IMAGE_WIDTH)
                            .corner_radius(10.0),
                    );
                }
                None => {
                    ui.add_sized([IMAGE_WIDTH, IMAGE_WIDTH * 1.4], egui::Spinner::new());
                }
            }
        }

        ui.vertical(|ui| {
            ui.set_max_width(320.0);
            if record.name != card.name {
                ui.weak(record.name.as_str());
            }
            field(ui, "Custo", record.mana_cost.as_deref().unwrap_or("-"));
            field(ui, "CMC", &record.cmc.to_string());
            field(ui, "Tipo", record.type_line.as_deref().unwrap_or("-"));
            field(
                ui,
                "Raridade",
                &format!("{} ({})", record.rarity, record.set_code),
            );

            ui.separator();
            if let Some(text) = &record.oracle_text {
                ui.label(text.as_str());
                ui.separator();
            }
            ui.weak(format!("Legal em: {}", record.legal_formats().join(", ")));
        });
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.strong(label);
        ui.label(value);
    });
}
