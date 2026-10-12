use super::app::MtgApp;
use eframe::{App, egui};
use rust_mtg::config::Config;
use rust_mtg::{card_db, locate, paths, player_log};
use std::path::{Path, PathBuf};

const MISSING_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 167, 38);

pub enum Startup {
    Setup(Setup),
    Ready(Box<MtgApp>),
}

pub struct Setup {
    config: Config,
    raw_dir: Option<PathBuf>,
    log_path: Option<PathBuf>,
    card_db: Option<PathBuf>,
    candidates: Vec<PathBuf>,
    error: Option<String>,
}

enum Action {
    Retry,
    PickLog,
    UseLog(PathBuf),
    PickCardDb,
}

impl Startup {
    pub fn new(
        ctx: &egui::Context,
        config: Config,
        log: Option<PathBuf>,
        raw_dir: Option<PathBuf>,
    ) -> Self {
        let mut setup = Setup {
            config,
            raw_dir,
            log_path: log,
            card_db: None,
            candidates: Vec::new(),
            error: None,
        };
        setup.detect();

        match setup.start(ctx) {
            Some(app) => Startup::Ready(Box::new(app)),
            None => Startup::Setup(setup),
        }
    }
}

impl App for Startup {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Startup::Ready(app) = self {
            app.logic(ctx, frame);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        match self {
            Startup::Ready(app) => app.ui(ui, frame),
            Startup::Setup(setup) => {
                if let Some(app) = setup.ui(ui) {
                    *self = Startup::Ready(Box::new(app));
                }
            }
        }
    }
}

impl Setup {
    fn detect(&mut self) {
        self.error = None;
        if self.log_path.is_none() {
            self.log_path = locate::find_log_path(None, &mut self.config);
        }
        self.candidates = match self.log_path {
            Some(_) => Vec::new(),
            None => paths::detect_log_files(),
        };

        if self.card_db.is_some() {
            return;
        }
        let Some(log_path) = self.log_path.clone() else {
            return;
        };
        match player_log::read_log(&log_path) {
            Ok(text) => {
                self.card_db =
                    locate::find_card_db(self.raw_dir.clone(), &mut self.config, &log_path, &text);
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn start(&mut self, ctx: &egui::Context) -> Option<MtgApp> {
        let (Some(log_path), Some(card_db)) = (self.log_path.clone(), self.card_db.clone()) else {
            return None;
        };
        if let Err(error) = self.config.save() {
            self.error = Some(format!("erro ao salvar a config: {error:#}"));
        }

        match MtgApp::new(ctx, self.config.clone(), log_path, card_db) {
            Ok(app) => Some(app),
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                None
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> Option<MtgApp> {
        let ctx = ui.ctx().clone();
        let action = egui::CentralPanel::default()
            .show(ui, |ui| self.contents(ui))
            .inner;

        match action? {
            Action::Retry => self.detect(),
            Action::PickLog => {
                if let Some(path) = paths::pick_log_file() {
                    self.use_log(path);
                }
            }
            Action::UseLog(path) => self.use_log(path),
            Action::PickCardDb => {
                if let Some(path) = card_db::pick_card_db() {
                    self.config.raw_dir = path.parent().map(Path::to_path_buf);
                    self.card_db = Some(path);
                }
            }
        }
        self.start(&ctx)
    }

    fn use_log(&mut self, path: PathBuf) {
        self.config.log_path = Some(path.clone());
        self.log_path = Some(path);
        self.card_db = None;
        self.detect();
    }

    fn contents(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;

        ui.set_max_width(680.0);
        ui.add_space(16.0);
        ui.heading("Antes de começar");
        ui.add_space(8.0);
        found_line(ui, "Player.log", self.log_path.as_deref());
        found_line(ui, "Banco de cartas do jogo", self.card_db.as_deref());
        ui.add_space(12.0);

        if self.log_path.is_none() {
            log_help(ui);
        } else if self.card_db.is_none() {
            card_db_help(ui);
        }

        if !self.candidates.is_empty() {
            ui.add_space(8.0);
            ui.label("Encontrei mais de um Player.log. Qual deles é o seu?");
            for candidate in &self.candidates {
                if ui.button(candidate.display().to_string()).clicked() {
                    action = Some(Action::UseLog(candidate.clone()));
                }
            }
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Procurar de novo").clicked() {
                action = Some(Action::Retry);
            }
            if ui.button("Escolher Player.log…").clicked() {
                action = Some(Action::PickLog);
            }
            if ui.button("Escolher banco de cartas…").clicked() {
                action = Some(Action::PickCardDb);
            }
        });

        if let Some(error) = &self.error {
            ui.add_space(8.0);
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
        action
    }
}

fn found_line(ui: &mut egui::Ui, label: &str, path: Option<&Path>) {
    match path {
        Some(path) => ui.label(format!("✔ {label}: {}", path.display())),
        None => ui.colored_label(MISSING_COLOR, format!("✖ {label}: não encontrado")),
    };
}

fn log_help(ui: &mut egui::Ui) {
    ui.label(
        "1. Abra o MTG Arena pelo menos uma vez e faça login: é o jogo que cria o Player.log.",
    );
    ui.label(
        "2. No jogo, ative Options → Account → \"Detailed Logs (Plugin Support)\" e reinicie o \
         jogo. Sem isso, os decks e os wildcards não aparecem aqui.",
    );
    ui.label(
        "3. Clique em \"Procurar de novo\". Se o jogo estiver instalado num lugar diferente, use \
         \"Escolher Player.log…\": ele fica em <prefixo do Wine>/drive_c/users/<usuário>/AppData/\
         LocalLow/Wizards Of The Coast/MTGA/Player.log (pela Steam, o prefixo é \
         steamapps/compatdata/2141910/pfx).",
    );
}

fn card_db_help(ui: &mut egui::Ui) {
    ui.label(
        "O banco de cartas (Raw_CardDatabase_*.mtga) é baixado pelo MTGA na primeira vez que ele \
         abre. Abra o jogo, espere terminar os downloads e clique em \"Procurar de novo\", ou \
         escolha o arquivo na pasta MTGA_Data/Downloads/Raw da instalação do jogo.",
    );
}
