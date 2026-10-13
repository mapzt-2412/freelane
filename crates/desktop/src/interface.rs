use super::*;
const ACCENT: egui::Color32 = egui::Color32::from_rgb(99, 82, 221);
const INK: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
const MUTED: egui::Color32 = egui::Color32::from_rgb(113, 116, 123);
const BORDER: egui::Color32 = egui::Color32::from_rgb(224, 225, 230);
#[derive(Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Display,
    Applications,
    Details,
}
#[derive(Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Hardware,
    Graphics,
    Files,
    Devices,
}
fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(16)
}
fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(
        egui::RichText::new(text)
            .color(egui::Color32::WHITE)
            .strong(),
    )
    .fill(ACCENT)
    .min_size(egui::vec2(110.0, 36.0))
}
fn icon(ui: &mut egui::Ui, os: GuestOs, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let color = match os {
        GuestOs::Windows => egui::Color32::from_rgb(42, 133, 245),
        GuestOs::Ubuntu => egui::Color32::from_rgb(237, 112, 49),
    };
    ui.painter().rect_filled(rect, 12, color);
    let painter = ui.painter();
    match os {
        GuestOs::Windows => {
            let panes = rect.shrink(size * 0.24);
            let gap = size * 0.045;
            let half = (panes.width() - gap) / 2.0;
            for row in 0..2 {
                for column in 0..2 {
                    let origin = panes.min
                        + egui::vec2(column as f32 * (half + gap), row as f32 * (half + gap));
                    painter.rect_filled(
                        egui::Rect::from_min_size(origin, egui::vec2(half, half)),
                        1,
                        egui::Color32::WHITE,
                    );
                }
            }
        }
        GuestOs::Ubuntu => {
            painter.circle_stroke(
                rect.center(),
                size * 0.19,
                egui::Stroke::new(size * 0.055, egui::Color32::WHITE),
            );
            for index in 0..3 {
                let angle = index as f32 * std::f32::consts::TAU / 3.0;
                let point = rect.center() + egui::vec2(angle.cos(), angle.sin()) * size * 0.26;
                painter.circle_filled(point, size * 0.085, color);
                painter.circle_filled(point, size * 0.058, egui::Color32::WHITE);
            }
        }
    }
}
pub fn install_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "FreeLane UI".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/DejaVuSans.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "FreeLane UI".into());
    #[cfg(target_os = "macos")]
    if let Ok(data) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts.font_data.insert(
            "macOS system".into(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "macOS system".into());
    }
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Light);
    ctx.global_style_mut(|style| {
        style.visuals.panel_fill = egui::Color32::from_rgb(248, 248, 250);
        style.visuals.window_fill = egui::Color32::WHITE;
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::WHITE;
        style.visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
        style.visuals.window_corner_radius = egui::CornerRadius::same(12);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(243, 244, 246);
        style.visuals.override_text_color = Some(INK);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(231, 225, 253);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, INK);
        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(239, 240, 243);
        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(226, 231, 239);
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(218, 210, 251);
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.spacing.button_padding = egui::vec2(13.0, 8.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
    });
}
impl Desktop {
    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.language;
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::WHITE)
                    .inner_margin(12),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("FreeLane")
                            .size(22.0)
                            .strong()
                            .color(ACCENT),
                    );
                    ui.separator();
                    ui.colored_label(MUTED, lang.text("Control Center"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(if self.language == Language::Vietnamese {
                                "Tiếng Việt"
                            } else {
                                "English"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.language,
                                    Language::Vietnamese,
                                    "Tiếng Việt",
                                );
                                ui.selectable_value(
                                    &mut self.language,
                                    Language::English,
                                    "English",
                                );
                            });
                    });
                });
            });
        egui::Panel::left("library")
            .default_size(230.0)
            .size_range(210.0..=300.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(244, 243, 249))
                    .inner_margin(16),
            )
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(lang.text("Virtual machines"))
                        .strong()
                        .color(MUTED),
                );
                ui.add_space(8.0);
                let mut chosen = None;
                let mut repair = None;
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 140.0).max(120.0))
                    .show(ui, |ui| {
                        for (path, name, os) in &self.library {
                            let selected = self.vm.as_ref().is_some_and(|vm| vm.path == *path);
                            let response = egui::Frame::new()
                                .fill(if selected {
                                    egui::Color32::from_rgb(230, 224, 251)
                                } else {
                                    egui::Color32::from_rgb(244, 243, 249)
                                })
                                .stroke(egui::Stroke::new(1.0, egui::Color32::TRANSPARENT))
                                .corner_radius(8)
                                .inner_margin(10)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        icon(ui, *os, 38.0);
                                        ui.vertical(|ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(name).strong(),
                                                )
                                                .truncate(),
                                            );
                                            ui.small(match os {
                                                GuestOs::Windows => "Windows · ARM64",
                                                GuestOs::Ubuntu => "Ubuntu · ARM64",
                                            });
                                        });
                                    });
                                })
                                .response;
                            if ui
                                .interact(
                                    response.rect,
                                    response.id.with("select"),
                                    egui::Sense::click(),
                                )
                                .clicked()
                                && self.pending.is_none()
                                && self.child.is_none()
                                && !self.settings_open
                            {
                                chosen = Some(path.clone());
                            }
                            if ui
                                .add_enabled(
                                    self.pending.is_none()
                                        && self.child.is_none()
                                        && !self.settings_open,
                                    egui::Button::new(lang.text("Locate VM")),
                                )
                                .clicked()
                            {
                                repair = Some(path.clone());
                            }
                            ui.add_space(4.0);
                        }
                    });
                if let Some(path) = repair {
                    self.repair_bookmark(ctx, path);
                }
                if let Some(path) = chosen {
                    self.draft_path = path.display().to_string();
                    self.open(ctx);
                }
                if ui
                    .add_enabled(
                        self.pending.is_none() && self.child.is_none() && !self.settings_open,
                        egui::Button::new(lang.text("Add virtual machine")),
                    )
                    .clicked()
                {
                    self.open_dialog = true;
                }
                if ui
                    .add_enabled(
                        self.pending.is_none() && self.child.is_none(),
                        egui::Button::new(lang.text("Create virtual machine")),
                    )
                    .clicked()
                {
                    self.wizard.open = true;
                }
                if ui.button(lang.text("Android & iOS")).clicked() {
                    self.mobile_open = true;
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(248, 248, 250))
                    .inner_margin(24),
            )
            .show(ui, |ui| {
                let viewport_height = ui.available_height();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.vm.is_some() {
                        self.machine_ui(ui, ctx, viewport_height);
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(75.0);
                            ui.horizontal(|ui| {
                                let offset = ((ui.available_width() - 150.0) / 2.0).max(0.0);
                                ui.add_space(offset);
                                icon(ui, GuestOs::Windows, 64.0);
                                icon(ui, GuestOs::Ubuntu, 64.0);
                            });
                            ui.add_space(24.0);
                            ui.heading(lang.text("Your next workspace starts here"));
                            ui.label(lang.text("Open a Windows or Ubuntu configuration to begin."));
                            ui.add_space(18.0);
                            if ui.add(primary(lang.text("Open virtual machine"))).clicked() {
                                self.open_dialog = true;
                            }
                        });
                    }
                    if self.pending.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                        });
                    }
                    if !self.library_notice.is_empty() {
                        ui.colored_label(
                            MUTED,
                            format!(
                                "{}: {}",
                                lang.text("Library not saved"),
                                self.library_notice
                            ),
                        );
                    }
                    if !self.notice.is_empty() {
                        ui.add_space(10.0);
                        ui.collapsing(lang.text("Notifications"), |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(100.0)
                                .show(ui, |ui| {
                                    ui.label(lang.message(&self.notice));
                                });
                        });
                    }
                });
            });
        if self.open_dialog {
            let mut open = true;
            egui::Window::new(lang.text("Open virtual machine"))
                .id(egui::Id::new("open-dialog"))
                .open(&mut open)
                .default_width(550.0)
                .default_pos(ctx.content_rect().center() - egui::vec2(275.0, 100.0))
                .show(ctx, |ui| {
                    ui.label(lang.text("Configuration file"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.draft_path)
                            .desired_width(f32::INFINITY)
                            .hint_text("/path/to/vm.toml"),
                    );
                    if ui
                        .add_enabled(
                            self.pending.is_none()
                                && self.child.is_none()
                                && !self.draft_path.trim().is_empty(),
                            primary(lang.text("Open")),
                        )
                        .clicked()
                    {
                        self.open(ctx);
                    }
                });
            self.open_dialog = open;
        }
        self.settings_ui(ctx);
    }
    fn machine_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, viewport_height: f32) {
        let content_top = ui.cursor().top();
        let lang = self.language;
        let vm = self.vm.as_ref().unwrap();
        let (name, os, cpus, memory, agent_enabled, path, acceleration, graphics) = (
            vm.name.clone(),
            vm.guest_os,
            vm.cpus,
            vm.memory,
            vm.guest_agent,
            vm.path.clone(),
            vm.acceleration,
            vm.config
                .runtime
                .as_ref()
                .map(|r| r.graphics)
                .unwrap_or_default(),
        );
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(235, 231, 253))
            .corner_radius(22)
            .inner_margin(24)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    icon(ui, os, 72.0);
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(lang.text(match os {
                                GuestOs::Windows => "Windows workspace",
                                GuestOs::Ubuntu => "Ubuntu workspace",
                            }))
                            .size(11.0)
                            .color(ACCENT)
                            .strong(),
                        );
                        ui.label(egui::RichText::new(name).size(30.0).strong());
                        ui.colored_label(
                            MUTED,
                            format!(
                                "{} · {}",
                                lang.text("Last observed state"),
                                lang.text(&self.status)
                            ),
                        );
                    });
                });
            });
        ui.add_space(12.0);
        let busy = self.pending.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy
                        && self.child.is_none()
                        && self.status != "running"
                        && self.status != "paused",
                    primary(lang.text("Start")),
                )
                .clicked()
            {
                self.start();
            }
            for (label, action, allowed) in [
                ("Pause", "pause", self.status == "running"),
                ("Resume", "resume", self.status == "paused"),
                (
                    "Shutdown",
                    "shutdown",
                    matches!(self.status.as_str(), "running" | "paused"),
                ),
                ("Refresh status", "status", true),
            ] {
                if !allowed {
                    continue;
                }
                if ui
                    .add_enabled(!busy, egui::Button::new(lang.text(label)))
                    .clicked()
                {
                    self.control(ctx, action);
                }
            }
            if ui
                .add_enabled(!busy, egui::Button::new(lang.text("Settings")))
                .clicked()
            {
                self.notice.clear();
                self.settings_open = true;
            }
        });
        ui.add_space(10.0);
        let width = ((ui.available_width() - 24.0) / 3.0 - 34.0).max(90.0);
        ui.horizontal_wrapped(|ui| {
            for (index, (label, value)) in [
                ("Virtual CPUs", cpus.to_string()),
                ("Memory", format!("{:.1} GiB", memory as f64 / 1024.0)),
                (
                    "Graphics",
                    lang.text(if graphics == qemu::Graphics::Virtio2d {
                        "Virtio GPU · 2D"
                    } else {
                        "Basic display"
                    })
                    .to_owned(),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let tint = [
                    egui::Color32::from_rgb(231, 239, 255),
                    egui::Color32::from_rgb(232, 245, 237),
                    egui::Color32::from_rgb(255, 240, 228),
                ][index];
                egui::Frame::new()
                    .fill(tint)
                    .corner_radius(16)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        ui.vertical(|ui| {
                            ui.colored_label(
                                MUTED,
                                egui::RichText::new(lang.text(label)).size(12.0),
                            );
                            ui.add_space(3.0);
                            ui.label(egui::RichText::new(value).strong().size(19.0));
                        });
                    });
            }
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.view, View::Display, lang.text("Display"));
            ui.selectable_value(
                &mut self.view,
                View::Applications,
                lang.text("Applications"),
            );
            ui.selectable_value(&mut self.view, View::Details, lang.text("Details"));
        });
        ui.add_space(10.0);
        match self.view {
            View::Display => {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new(lang.text("Refresh display")))
                        .on_hover_text(lang.text("Snapshots, not live video"))
                        .clicked()
                    {
                        self.capture(ctx, true);
                    }
                    if ui
                        .checkbox(&mut self.auto_display, lang.text("Refresh every 5 seconds"))
                        .changed()
                    {
                        self.display_at = None;
                    }
                });
                let height =
                    (viewport_height - (ui.cursor().top() - content_top) - 85.0).max(150.0);
                egui::Frame::new()
                    .fill(if self.texture.is_some() {
                        egui::Color32::from_rgb(29, 36, 49)
                    } else {
                        egui::Color32::WHITE
                    })
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .corner_radius(12)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if let Some(texture) = &self.texture {
                            let frame = ui.add(
                                egui::Image::new(texture)
                                    .max_size(egui::vec2(ui.available_width(), height - 28.0))
                                    .sense(egui::Sense::click()),
                            );
                            if frame.clicked() && self.pending.is_none() && self.status == "running"
                            {
                                if let Some(point) = frame.interact_pointer_pos() {
                                    let x = ((point.x - frame.rect.left()) / frame.rect.width()
                                        * 32767.0)
                                        .clamp(0.0, 32767.0)
                                        .round() as u16;
                                    let y = ((point.y - frame.rect.top()) / frame.rect.height()
                                        * 32767.0)
                                        .clamp(0.0, 32767.0)
                                        .round() as u16;
                                    self.click(ctx, x, y);
                                }
                            }
                        } else {
                            ui.allocate_ui(egui::vec2(ui.available_width(), height - 28.0), |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.add_space(((height - 230.0) / 2.0).max(12.0));
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(150.0, 100.0),
                                        egui::Sense::hover(),
                                    );
                                    let painter = ui.painter();
                                    painter.circle_filled(
                                        rect.center() + egui::vec2(-35.0, -5.0),
                                        48.0,
                                        egui::Color32::from_rgb(238, 234, 253),
                                    );
                                    painter.circle_filled(
                                        rect.center() + egui::vec2(40.0, 0.0),
                                        35.0,
                                        egui::Color32::from_rgb(226, 239, 253),
                                    );
                                    let screen = egui::Rect::from_center_size(
                                        rect.center() - egui::vec2(0.0, 8.0),
                                        egui::vec2(100.0, 65.0),
                                    );
                                    painter.rect_filled(screen, 10, egui::Color32::WHITE);
                                    painter.rect_stroke(
                                        screen,
                                        10,
                                        egui::Stroke::new(2.0, ACCENT),
                                        egui::StrokeKind::Inside,
                                    );
                                    painter.line_segment(
                                        [
                                            rect.center() + egui::vec2(0.0, 25.0),
                                            rect.center() + egui::vec2(0.0, 38.0),
                                        ],
                                        egui::Stroke::new(3.0, ACCENT),
                                    );
                                    painter.line_segment(
                                        [
                                            rect.center() + egui::vec2(-18.0, 39.0),
                                            rect.center() + egui::vec2(18.0, 39.0),
                                        ],
                                        egui::Stroke::new(3.0, ACCENT),
                                    );
                                    ui.add_space(14.0);
                                    ui.label(
                                        egui::RichText::new(lang.text("No display snapshot yet"))
                                            .size(20.0)
                                            .strong(),
                                    );
                                });
                            });
                        }
                    });
            }
            View::Applications => {
                card().show(ui, |ui| {
                    ui.heading(lang.text("Guest connection"));
                    if ui
                        .add_enabled(
                            agent_enabled && !busy,
                            egui::Button::new(lang.text("Check connection")),
                        )
                        .clicked()
                    {
                        self.guest_request(ctx, false);
                    }
                    ui.label(lang.text("Executable path"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.app_path)
                            .desired_width(f32::INFINITY)
                            .hint_text(if os == GuestOs::Ubuntu {
                                "/absolute/path/to/app"
                            } else {
                                r"C:\Windows\System32\notepad.exe"
                            }),
                    );
                    ui.label(lang.text("Arguments (one per line)"));
                    ui.add(
                        egui::TextEdit::multiline(&mut self.app_arguments)
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                    if ui
                        .add_enabled(
                            agent_enabled && !busy && !self.app_path.trim().is_empty(),
                            primary(lang.text("Launch app")),
                        )
                        .clicked()
                    {
                        self.guest_request(ctx, true);
                    }
                    if os == GuestOs::Windows {
                        ui.separator();
                        ui.heading(lang.text("Independent app window & Dock"));
                        ui.label(lang.text("App name"));
                        ui.add(egui::TextEdit::singleline(&mut self.app_name).hint_text("Notepad"));
                        ui.label(lang.text("New app profile path"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.app_profile_output)
                                .desired_width(f32::INFINITY)
                                .hint_text("/absolute/path/to/notepad.app.toml"),
                        );
                        let ready = agent_enabled
                            && !busy
                            && !self.app_name.trim().is_empty()
                            && !self.app_profile_output.trim().is_empty()
                            && !self.app_path.trim().is_empty();
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    ready,
                                    egui::Button::new(lang.text("Save app profile")),
                                )
                                .clicked()
                            {
                                self.save_app_profile(ctx, false);
                            }
                            if ui
                                .add_enabled(
                                    ready,
                                    egui::Button::new(lang.text("Save & open separate window")),
                                )
                                .clicked()
                            {
                                self.save_app_profile(ctx, true);
                            }
                        });
                        if ui
                            .add_enabled(
                                !busy && !self.app_profile_output.trim().is_empty(),
                                egui::Button::new(lang.text("Open saved app profile")),
                            )
                            .clicked()
                        {
                            self.open_saved_app(ctx);
                        }
                        ui.label(lang.text("New Dock .app path (Mac only)"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.dock_output)
                                .desired_width(f32::INFINITY)
                                .hint_text("/Users/you/Applications/Windows Notepad.app"),
                        );
                        if ui
                            .add_enabled(
                                cfg!(target_os = "macos")
                                    && !busy
                                    && !self.app_profile_output.trim().is_empty()
                                    && !self.dock_output.trim().is_empty(),
                                egui::Button::new(lang.text("Export Dock launcher")),
                            )
                            .clicked()
                        {
                            self.export_app_dock(ctx);
                        }
                    }
                    if !agent_enabled {
                        ui.small(
                            lang.text("Install and run the guest agent to launch applications."),
                        );
                    }
                });
            }
            View::Details => {
                card().show(ui, |ui| {
                    ui.label(lang.text("Configuration path"));
                    ui.label(path.display().to_string());
                    ui.label(lang.text("Guest agent"));
                    ui.label(lang.text(&self.agent_status));
                    ui.label(lang.text("Configured acceleration"));
                    ui.label(lang.text(acceleration));
                    ui.collapsing(lang.text("Advanced controls"), |ui| {
                        ui.small(lang.text("Force stop discards RAM and can lose unsaved data."));
                        if ui
                            .add_enabled(!busy, egui::Button::new(lang.text("Force stop")))
                            .clicked()
                        {
                            self.control(ctx, "stop");
                        }
                    });
                });
            }
        }
    }
    pub fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let lang = self.language;
        let mut open = true;
        let mut save = false;
        let mut import = false;
        let mut export = false;
        egui::Window::new(lang.text("Virtual machine settings")).id(egui::Id::new("settings"))
            .open(&mut open).default_pos(ctx.content_rect().center() - egui::vec2(305.0, 240.0)).default_width(610.0).default_height(480.0).show(ctx, |ui| {
            ui.small(lang.text("Changes apply on the next start. Shut down before saving."));
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.settings_tab, SettingsTab::Hardware, lang.text("Hardware"));
                ui.selectable_value(&mut self.settings_tab, SettingsTab::Graphics, lang.text("Graphics"));
                ui.selectable_value(&mut self.settings_tab, SettingsTab::Files, lang.text("Files"));
                ui.selectable_value(&mut self.settings_tab, SettingsTab::Devices, lang.text("Devices"));
            }); ui.separator();
            egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                let Some(config) = self.settings.as_mut() else { return };
                match self.settings_tab {
                    SettingsTab::Hardware => {
                        ui.label(lang.text("Name")); ui.text_edit_singleline(&mut config.name);
                        ui.add(egui::Slider::new(&mut config.cpus, 1..=64).text(lang.text("Virtual CPUs")));
                        ui.horizontal(|ui| { ui.label(lang.text("RAM (MiB)")); ui.add(egui::DragValue::new(&mut config.memory_mib).range(2048..=1_048_576).speed(256)); });
                        if let Some(runtime) = config.runtime.as_mut() {
                            ui.label(lang.text("Acceleration")); ui.horizontal_wrapped(|ui| {
                                ui.selectable_value(&mut runtime.accelerator,qemu::Accelerator::Hvf,lang.text("Mac hardware · HVF"));
                                ui.selectable_value(&mut runtime.accelerator,qemu::Accelerator::Tcg,lang.text("Software · TCG"));
                            });
                            if runtime.accelerator == qemu::Accelerator::Hvf { runtime.tcg_threading = None; }
                            else { ui.horizontal(|ui| {
                                ui.selectable_value(&mut runtime.tcg_threading,None,lang.text("Default"));
                                ui.selectable_value(&mut runtime.tcg_threading,Some(qemu::TcgThreading::Single),lang.text("Single"));
                                ui.selectable_value(&mut runtime.tcg_threading,Some(qemu::TcgThreading::Multi),lang.text("Multi"));
                            }); }
                        }
                    }
                    SettingsTab::Graphics => {
                        if let Some(runtime) = config.runtime.as_mut() {
                            ui.selectable_value(&mut runtime.graphics,qemu::Graphics::Ramfb,lang.text("Basic display"));
                            ui.add_enabled_ui(config.guest_os == GuestOs::Ubuntu, |ui| { ui.selectable_value(&mut runtime.graphics,qemu::Graphics::Virtio2d,lang.text("Virtio GPU · 2D")).on_hover_text(lang.text("Virtio 2D requires an Ubuntu driver. It does not enable Metal, 3D or Windows GPU acceleration.")); });
                            ui.checkbox(&mut runtime.headless,lang.text("Headless · hide native VM window"));
                        }
                    }
                    SettingsTab::Files => {
                        ui.label(format!("{}: {}",lang.text("Guest disk"),config.disk.display()));
                        ui.label(lang.text("Installer / CD image (blank to detach)")); ui.add(egui::TextEdit::singleline(&mut self.installer_path).desired_width(f32::INFINITY));
                        ui.label(lang.text("Host folder (blank to disable)")); ui.add(egui::TextEdit::singleline(&mut self.shared_path).desired_width(f32::INFINITY));
                        if let Some(runtime) = config.runtime.as_mut() {
                            ui.horizontal_wrapped(|ui| {
                                ui.selectable_value(&mut runtime.share_mode,qemu::ShareMode::ReadonlyUsb,lang.text("Read-only USB"));
                                ui.add_enabled_ui(config.guest_os == GuestOs::Ubuntu, |ui| { ui.selectable_value(&mut runtime.share_mode,qemu::ShareMode::Virtio9p,lang.text("Live read/write · Ubuntu")); });
                            });
                            ui.small(lang.text(if runtime.share_mode == qemu::ShareMode::ReadonlyUsb { "Keep FAT shares under 504 MiB and unchanged while the VM runs." } else { "Guest writes modify host files. Mount the share inside Ubuntu; see docs/sharing.md." }));
                        }
                    }
                    SettingsTab::Devices => {
                        if let Some(runtime) = config.runtime.as_mut() {
                            ui.checkbox(&mut runtime.network,lang.text("NAT networking"));
                            if runtime.network { ui.horizontal_wrapped(|ui| {
                                ui.label(lang.text("Network adapter"));
                                ui.selectable_value(&mut runtime.network_device,qemu::NetworkDevice::Virtio,"Virtio PCI");
                                ui.selectable_value(&mut runtime.network_device,qemu::NetworkDevice::Usb,lang.text("USB network"));
                            }); }
                            ui.checkbox(&mut runtime.guest_agent,lang.text("Guest agent channel"));
                            ui.checkbox(&mut runtime.serial_console,lang.text("Private serial console"));
                            ui.separator(); ui.label(lang.text("Physical USB devices")).on_hover_text(lang.text("Inspect IDs with scripts/list-usb.py. Device capture needs OS access; addresses may change."));
                            let mut remove = None;
                            for (index,device) in runtime.usb_devices.iter_mut().enumerate() { ui.horizontal_wrapped(|ui| {
                                ui.label("Bus"); ui.add(egui::DragValue::new(&mut device.bus).range(1..=255));
                                ui.label(lang.text("Address")); ui.add(egui::DragValue::new(&mut device.address).range(1..=127));
                                ui.label("VID"); ui.add(egui::DragValue::new(&mut device.vendor_id).hexadecimal(4,false,true));
                                ui.label("PID"); ui.add(egui::DragValue::new(&mut device.product_id).hexadecimal(4,false,true));
                                if ui.button(lang.text("Remove")).clicked() { remove = Some(index); }
                            }); }
                            if let Some(index) = remove { runtime.usb_devices.remove(index); }
                            if ui.add_enabled(runtime.usb_devices.len()<16,egui::Button::new(lang.text("Add USB device"))).clicked() { runtime.usb_devices.push(qemu::UsbHostDevice{bus:1,address:1,vendor_id:0,product_id:0}); }
                        }
                    }
                }
            }); ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(self.pending.is_none() && self.child.is_none(),primary(lang.text("Save changes"))).clicked() { save = true; }
                if ui.add_enabled(self.pending.is_none(),egui::Button::new(lang.text("Import settings"))).clicked(){import=true;}
                if ui.add_enabled(self.pending.is_none(),egui::Button::new(lang.text("Export saved settings"))).clicked(){export=true;}
            });
            if !self.notice.is_empty() { ui.label(lang.message(&self.notice)); }
        });
        self.settings_open = open;
        if save {
            self.save_settings(ctx);
        } else if import {
            self.import_settings_ui(ctx);
        } else if export {
            self.export_settings_ui(ctx);
        }
    }
}
