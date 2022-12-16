use egui::widget_text;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { label, value } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Navigation");

            ui.horizontal(|ui| {
                let button = egui::Button::new("Grafenau")
                    .fill(egui::Color32::from_rgb(50,50,50));
                ui.add(button);
                let button = egui::Button::new("House")
                    .fill(egui::Color32::from_rgb(50,50,50));
                ui.add(button);
            });


            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                let box_size = egui::Vec2{x:100.0, y:100.0};
                let button = egui::Button::new("OG")
                    .fill(egui::Color32::from_rgb(128,0,0))
                    .min_size(box_size);
                ui.add(button);
                let button = egui::Button::new("EG")
                    .fill(egui::Color32::from_rgb(0,128,0))
                    .min_size(box_size);
                ui.add(button);
                let button = egui::Button::new("KG")
                    .fill(egui::Color32::from_rgb(0,0,128))
                    .min_size(box_size);
                ui.add(button);

            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("Controls");

            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(value, 0.0..=4000.0).text("Hue"));
                let mut auto_hue = true;
                ui.add(egui::Checkbox::new(&mut auto_hue, "AutoHue"))
            });
            ui.add(egui::Button::new("All-Off"));
            ui.add(egui::Button::new("Tuer EG"));
            ui.add(egui::Button::new("Tuer KG"));
        });
    }
}
