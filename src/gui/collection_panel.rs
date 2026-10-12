use eframe::egui;

pub struct CollectionInfo<'a> {
    pub has_collection: bool,
    pub syncing: bool,
    pub status: &'a str,
}

#[derive(Clone, Copy)]
pub enum CollectionAction {
    Sync,
}

pub fn controls(ui: &mut egui::Ui, info: &CollectionInfo) -> Option<CollectionAction> {
    ui.weak(info.status);

    if !info.has_collection {
        ui.weak("A coleção é lida direto do MTGA: abra o jogo e faça login.");
    }

    if info.syncing {
        ui.spinner();
        return None;
    }

    ui.button("Sincronizar coleção agora")
        .clicked()
        .then_some(CollectionAction::Sync)
}
