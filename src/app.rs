use crate::escpos::parse_escpos;
use crate::hex_dump::pretty_hex;
use crate::model::{Align, CodePage, CommandType, Control, PaperWidth, PrinterState};
use crate::tcp_capture::TcpCapture;
use crate::tray::SystemTray;
use crate::window_control::WindowControl;
use eframe::egui;
use qrcode::types::Color;
use qrcode::{EcLevel, QrCode};
use rfd::FileDialog;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;
use std::time::{Duration, Instant};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Preview,
    Full,
}

#[derive(Debug, Clone)]
struct JobEntry {
    id: u64,
    label: String,
    created_at: Instant,

    full_bytes: Vec<u8>,
    display_bytes: Vec<u8>,
    parsed_commands: Vec<(PrinterState, CommandType)>,

    sim_active: bool,
    sim_started_at: Option<Instant>,
    sim_sent: usize,
}

pub struct EscPosViewer {
    jobs: Vec<JobEntry>,
    active_job_idx: Option<usize>,
    next_job_id: u64,

    max_jobs: usize,
    auto_prune_by_age: bool,
    prune_after: Duration,
    auto_scroll_on_print: bool,
    paper_width: PaperWidth,
    last_paper_width: PaperWidth,
    did_apply_initial_window_size: bool,
    did_apply_initial_window_position: bool,
    show_debug_controls: bool,
    show_debug_panels: bool,
    ui_mode: UiMode,
    last_ui_mode: UiMode,
    codepage: CodePage,
    texture_cache: HashMap<u64, egui::TextureHandle>,

    tcp_capture: Option<TcpCapture>,
    tcp_last_error: Option<String>,
    tcp_enabled: bool,

    tray: Option<SystemTray>,
    tray_error: Option<String>,
    pending_hide_to_tray: bool,
    hidden_to_tray: bool,

    window: WindowControl,

    simulate_printing: bool,
    sim_bytes_per_sec: usize,
}

impl Default for EscPosViewer {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            active_job_idx: None,
            next_job_id: 1,

            max_jobs: 25,
            auto_prune_by_age: false,
            prune_after: Duration::from_secs(60 * 60 * 2),
            auto_scroll_on_print: true,
            paper_width: PaperWidth::W58mm,
            last_paper_width: PaperWidth::W58mm,
            did_apply_initial_window_size: false,
            did_apply_initial_window_position: false,
            show_debug_controls: false,
            show_debug_panels: false,
            ui_mode: UiMode::Preview,
            last_ui_mode: UiMode::Preview,
            codepage: CodePage::Utf8Lossy,
            texture_cache: HashMap::new(),
            tcp_capture: None,
            tcp_last_error: None,
            tcp_enabled: true,

            tray: None,
            tray_error: None,
            pending_hide_to_tray: false,
            hidden_to_tray: false,

            window: WindowControl::default(),

            simulate_printing: true,
            sim_bytes_per_sec: 1_000,
        }
    }
}

impl EscPosViewer {
    fn format_age_short(d: Duration) -> String {
        let secs = d.as_secs();
        if secs < 60 {
            return format!("{}s", secs);
        }
        let mins = secs / 60;
        if mins < 60 {
            return format!("{}m", mins);
        }
        let hours = mins / 60;
        format!("{}h", hours)
    }

    fn ui_job_tabs(&mut self, ui: &mut egui::Ui) {
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            if ui.button("🧹 Limpiar historial").clicked() {
                self.jobs.clear();
                self.active_job_idx = None;
            }

            ui.checkbox(&mut self.auto_scroll_on_print, "Auto-scroll al imprimir");

            ui.add(egui::Slider::new(&mut self.max_jobs, 1..=100).text("Máx jobs"));

            ui.checkbox(&mut self.auto_prune_by_age, "Autolimpieza por edad");
            if self.auto_prune_by_age {
                let mut mins = (self.prune_after.as_secs() / 60).max(1);
                ui.add(egui::Slider::new(&mut mins, 1..=24 * 60).text("min"));
                self.prune_after = Duration::from_secs(mins * 60);
            }
        });

        if self.jobs.is_empty() {
            ui.label(egui::RichText::new("(sin trabajos aún)").weak());
            return;
        }

        let mut to_close: Option<usize> = None;
        egui::ScrollArea::horizontal()
            .id_salt("job_tabs_scroll")
            .max_height(34.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let now = Instant::now();
                    for (idx, job) in self.jobs.iter().enumerate() {
                        let selected = self.active_job_idx == Some(idx);
                        let age = now.duration_since(job.created_at);
                        let mut title = job.label.clone();
                        const MAX: usize = 26;
                        if title.chars().count() > MAX {
                            title = title.chars().take(MAX).collect::<String>();
                            title.push('…');
                        }

                        let tab_text = format!(
                            "#{} {} ({} · {}b)",
                            job.id,
                            title,
                            Self::format_age_short(age),
                            job.full_bytes.len()
                        );

                        if ui.selectable_label(selected, tab_text).clicked() {
                            self.active_job_idx = Some(idx);
                        }

                        if ui.small_button("✕").on_hover_text("Cerrar").clicked() {
                            to_close = Some(idx);
                        }
                    }
                });
            });

        if let Some(idx) = to_close {
            self.jobs.remove(idx);
            if self.jobs.is_empty() {
                self.active_job_idx = None;
            } else if let Some(active) = self.active_job_idx {
                if idx == active {
                    self.active_job_idx = Some(active.saturating_sub(1).min(self.jobs.len() - 1));
                } else if idx < active {
                    self.active_job_idx = Some(active - 1);
                }
            }
        }
    }
    fn active_job(&self) -> Option<&JobEntry> {
        self.active_job_idx
            .and_then(|idx| self.jobs.get(idx))
    }

    fn active_job_mut(&mut self) -> Option<&mut JobEntry> {
        let idx = self.active_job_idx?;
        self.jobs.get_mut(idx)
    }

    fn stop_active_simulation_show_full(&mut self) {
        let codepage = self.codepage;
        let Some(job) = self.active_job_mut() else {
            return;
        };
        if !job.sim_active {
            return;
        }
        job.sim_active = false;
        job.sim_started_at = None;
        job.display_bytes = job.full_bytes.clone();
        job.parsed_commands = parse_escpos(&job.display_bytes, codepage);
        job.sim_sent = job.display_bytes.len();
    }

    fn prune_jobs(&mut self) {
        let active_id = self.active_job().map(|j| j.id);

        if self.jobs.is_empty() {
            self.active_job_idx = None;
            return;
        }

        // Primero por edad (opcional)
        if self.auto_prune_by_age {
            let now = Instant::now();
            self.jobs.retain(|j| now.duration_since(j.created_at) <= self.prune_after);
        }

        // Luego por límite de cantidad (siempre)
        if self.jobs.len() > self.max_jobs {
            let remove_count = self.jobs.len() - self.max_jobs;
            self.jobs.drain(0..remove_count);
        }

        // Reajustar active_job_idx intentando mantener el mismo id.
        if self.jobs.is_empty() {
            self.active_job_idx = None;
            return;
        }

        if let Some(id) = active_id {
            if let Some(idx) = self.jobs.iter().position(|j| j.id == id) {
                self.active_job_idx = Some(idx);
                return;
            }
        }

        self.active_job_idx = Some(self.jobs.len() - 1);
    }

    fn push_new_job(&mut self, label: String, full_data: Vec<u8>) {
        // Si hay una simulación activa, la cerramos mostrando el job completo.
        self.stop_active_simulation_show_full();

        let id = self.next_job_id;
        self.next_job_id = self.next_job_id.saturating_add(1);

        let mut job = JobEntry {
            id,
            label,
            created_at: Instant::now(),
            full_bytes: full_data,
            display_bytes: Vec::new(),
            parsed_commands: Vec::new(),
            sim_active: false,
            sim_started_at: None,
            sim_sent: 0,
        };

        if self.simulate_printing {
            job.sim_active = true;
            job.sim_started_at = Some(Instant::now());
            job.display_bytes = Vec::with_capacity(job.full_bytes.len());
            job.parsed_commands.clear();
            job.sim_sent = 0;
        } else {
            job.display_bytes = job.full_bytes.clone();
            job.parsed_commands = parse_escpos(&job.display_bytes, self.codepage);
            job.sim_sent = job.display_bytes.len();
        }

        self.jobs.push(job);
        self.active_job_idx = Some(self.jobs.len() - 1);
        self.prune_jobs();
    }

    fn target_window_width_px(paper_width: PaperWidth) -> f32 {
        match paper_width {
            PaperWidth::W58mm => 375.0,
            PaperWidth::W80mm => 480.0,
        }
    }

    fn request_window_width(ctx: &egui::Context, width_px: f32) {
        // Mantener la altura actual cuando sea posible.
        let height_px: f32 = ctx
            .input(|i| i.viewport().inner_rect.map(|r| r.height()))
            .unwrap_or(600.0);

        // Nota: en egui 0.29 el comando se llama InnerSize.
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            width_px, height_px,
        )));
    }

    fn draw_printing_reveal_effect(ui: &mut egui::Ui, ticket_rect: egui::Rect, progress: f32) {
        let progress = progress.max(0.0).min(1.0);

        let y = egui::lerp(ticket_rect.top()..=ticket_rect.bottom(), progress);
        let y = y.max(ticket_rect.top()).min(ticket_rect.bottom());

        let painter = ui.painter();

        // Máscara sutil debajo de la barra (zona aún "no impresa").
        let mask_base = ui.visuals().faint_bg_color;
        let mask = egui::Color32::from_rgba_unmultiplied(
            mask_base.r(),
            mask_base.g(),
            mask_base.b(),
            110,
        );
        let mask_rect = egui::Rect::from_min_max(
            egui::pos2(ticket_rect.left(), y),
            egui::pos2(ticket_rect.right(), ticket_rect.bottom()),
        );
        painter.rect_filled(mask_rect, 0.0, mask);

        // Barra de "escaneo".
        let bar_h = 6.0;
        let bar_base = ui.visuals().selection.bg_fill;
        let bar = egui::Color32::from_rgba_unmultiplied(
            bar_base.r(),
            bar_base.g(),
            bar_base.b(),
            220,
        );

        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(ticket_rect.left(), (y - bar_h * 0.5).max(ticket_rect.top())),
            egui::pos2(ticket_rect.right(), (y + bar_h * 0.5).min(ticket_rect.bottom())),
        );
        painter.rect_filled(bar_rect, 0.0, bar);

        let stroke = ui.visuals().widgets.active.fg_stroke;
        painter.rect_stroke(bar_rect, 0.0, egui::Stroke::new(1.0, stroke.color));
    }
    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        // Windows: ocultar de verdad (sale del taskbar) via Win32 + WS_EX_TOOLWINDOW.
        // Otros OS: fallback a minimizar.
        #[cfg(target_os = "windows")]
        {
            let _ = ctx; // no usado
            self.window.hide_to_tray();
            return;
        }

        #[cfg(not(target_os = "windows"))]
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn tick_simulation(&mut self) {
        let bytes_per_sec = self.sim_bytes_per_sec;
        let codepage = self.codepage;
        let Some(job) = self.active_job_mut() else {
            return;
        };
        if !job.sim_active {
            return;
        }
        let Some(start) = job.sim_started_at else {
            return;
        };

        let elapsed = start.elapsed().as_secs_f32();
        let target = (elapsed * bytes_per_sec as f32) as usize;
        let target = target.min(job.full_bytes.len());

        if target > job.sim_sent {
            job.display_bytes
                .extend_from_slice(&job.full_bytes[job.sim_sent..target]);
            job.sim_sent = target;
            job.parsed_commands = parse_escpos(&job.display_bytes, codepage);
        }

        if job.sim_sent >= job.full_bytes.len() {
            job.sim_active = false;
            job.sim_started_at = None;
        }
    }

    fn set_tcp_capture(&mut self, enabled: bool, repaint_ctx: Option<egui::Context>) {
        if enabled {
            if self.tcp_capture.is_some() {
                return;
            }
            match TcpCapture::start("127.0.0.1:9100", repaint_ctx, Some(self.window.clone())) {
                Ok(capture) => {
                    self.tcp_capture = Some(capture);
                    self.tcp_last_error = None;
                }
                Err(e) => {
                    self.tcp_last_error = Some(format!(
                        "No se pudo escuchar 127.0.0.1:9100 ({})",
                        e
                    ));
                    self.tcp_capture = None;
                }
            }
        } else if let Some(mut cap) = self.tcp_capture.take() {
            cap.stop();
            self.tcp_capture = None;
        }
    }

    fn try_load_path(&mut self, path: &Path) {
        if let Ok(data) = fs::read(path) {
            self.push_new_job(path.display().to_string(), data);
        }
    }

    fn reparse_all_jobs(&mut self) {
        for job in &mut self.jobs {
            if job.display_bytes.is_empty() {
                job.parsed_commands.clear();
                continue;
            }
            job.parsed_commands = parse_escpos(&job.display_bytes, self.codepage);
        }
    }

    fn debug_label_for_control(control: &Control) -> String {
        match control {
            Control::Newline => "LF".to_string(),
            Control::Init => "ESC @ (INIT)".to_string(),
            Control::Bold(on) => format!("ESC E (BOLD={})", on),
            Control::Align(align) => format!("ESC a (ALIGN={:?})", align),
            Control::Size { raw, width, height } => {
                format!("GS ! (SIZE raw={:02X} w={} h={})", raw, width, height)
            }
            Control::Cut => "GS V (CUT)".to_string(),
            Control::RasterImage {
                m,
                width_bytes,
                height,
                data,
            } => {
                format!(
                    "GS v 0 (IMG m={:02X} {}x{} bytes={})",
                    m,
                    (*width_bytes as usize) * 8,
                    *height as usize,
                    data.len()
                )
            }
            Control::Qr {
                model,
                module_size,
                ecc,
                data,
            } => format!(
                "QR (model={} size={} ecc={} bytes={})",
                model,
                module_size,
                ecc,
                data.len()
            ),
            Control::Barcode { m, data } => {
                format!("GS k (BARCODE m={:02X} bytes={})", m, data.len())
            }
            Control::EscUnknown(b) => format!("ESC {:02X} (?)", b),
            Control::GsUnknown(b) => format!("GS {:02X} (?)", b),
        }
    }

    fn base_columns(paper_width: PaperWidth) -> usize {
        match paper_width {
            PaperWidth::W58mm => 32,
            PaperWidth::W80mm => 48,
        }
    }

    fn effective_columns(paper_width: PaperWidth, state: &PrinterState) -> usize {
        let base = Self::base_columns(paper_width);
        let div = state.char_width_mul.max(1) as usize;
        (base / div).max(1)
    }

    fn same_line_style(a: &PrinterState, b: &PrinterState) -> bool {
        a.is_bold == b.is_bold
            && a.alignment == b.alignment
            && a.char_width_mul == b.char_width_mul
            && a.char_height_mul == b.char_height_mul
    }

    fn nbsp_pad(count: usize) -> String {
        // NBSP para que egui no "coma" el padding inicial.
        "\u{00A0}".repeat(count)
    }

    fn split_and_wrap(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![text.to_string()];
        }

        let mut out = Vec::new();
        let mut current = String::new();
        let mut col = 0usize;

        for ch in text.chars() {
            if ch == '\n' {
                out.push(current);
                current = String::new();
                col = 0;
                continue;
            }

            if col >= width {
                out.push(current);
                current = String::new();
                col = 0;
            }

            current.push(ch);
            col += 1;
        }

        if !current.is_empty() {
            out.push(current);
        }

        if out.is_empty() {
            out.push(String::new());
        }

        out
    }

    fn emit_text_with_columns(
        ui: &mut egui::Ui,
        paper_width: PaperWidth,
        state: &PrinterState,
        text: &str,
    ) {
        let cols = Self::effective_columns(paper_width, state);
        let lines = Self::split_and_wrap(text, cols);

        for line in lines {
            let len = line.chars().count();
            let pad = if len >= cols {
                0
            } else {
                match state.alignment {
                    Align::Left => 0,
                    Align::Center => (cols - len) / 2,
                    Align::Right => cols - len,
                }
            };

            let mut display = String::new();
            display.push_str(&Self::nbsp_pad(pad));
            display.push_str(&line);

            let mut rich_text = egui::RichText::new(display)
                .color(egui::Color32::BLACK)
                .family(egui::FontFamily::Monospace)
                .size(14.0 * state.char_height_mul.max(1) as f32);

            if state.is_bold {
                rich_text = rich_text.strong();
            }

            ui.add(egui::Label::new(rich_text));
        }
    }

    fn hash_key<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn raster_to_image(width_bytes: u16, height: u16, data: &[u8]) -> Option<egui::ColorImage> {
        let width_bits = (width_bytes as usize).checked_mul(8)?;
        let height = height as usize;
        if width_bits == 0 || height == 0 {
            return None;
        }
        let expected = (width_bytes as usize).saturating_mul(height);
        if data.len() < expected {
            return None;
        }

        let mut pixels = vec![egui::Color32::WHITE; width_bits * height];

        for y in 0..height {
            let row = &data[y * width_bytes as usize..(y + 1) * width_bytes as usize];
            for (xb, byte) in row.iter().enumerate() {
                for bit in 0..8 {
                    let is_black = (byte & (1 << (7 - bit))) != 0;
                    let x = xb * 8 + bit;
                    let idx = y * width_bits + x;
                    if is_black {
                        pixels[idx] = egui::Color32::BLACK;
                    }
                }
            }
        }

        Some(egui::ColorImage {
            size: [width_bits, height],
            pixels,
        })
    }

    fn ecc_to_level(ecc: u8) -> EcLevel {
        match ecc {
            48 => EcLevel::L,
            49 => EcLevel::M,
            50 => EcLevel::Q,
            51 => EcLevel::H,
            _ => EcLevel::M,
        }
    }

    fn qr_to_image(data: &[u8], ecc: u8, module_size: u8) -> Option<egui::ColorImage> {
        let ec_level = Self::ecc_to_level(ecc);
        let code = QrCode::with_error_correction_level(data, ec_level).ok()?;
        let width = code.width();
        if width == 0 {
            return None;
        }

        let module = (module_size as usize).clamp(1, 16);
        let quiet = 4usize;
        let out_w = (width + 2 * quiet) * module;
        let out_h = out_w;

        let mut pixels = vec![egui::Color32::WHITE; out_w * out_h];

        let colors = code.to_colors();
        for y in 0..width {
            for x in 0..width {
                let c = colors[y * width + x];
                if c == Color::Dark {
                    let base_x = (x + quiet) * module;
                    let base_y = (y + quiet) * module;
                    for dy in 0..module {
                        for dx in 0..module {
                            let idx = (base_y + dy) * out_w + (base_x + dx);
                            pixels[idx] = egui::Color32::BLACK;
                        }
                    }
                }
            }
        }

        Some(egui::ColorImage {
            size: [out_w, out_h],
            pixels,
        })
    }

    fn show_image_scaled(
        ui: &mut egui::Ui,
        cache: &mut HashMap<u64, egui::TextureHandle>,
        key: u64,
        image: egui::ColorImage,
        target_width: f32,
    ) {
        let tex = cache
            .entry(key)
            .or_insert_with(|| {
                ui.ctx().load_texture(
                    format!("tex_{key}"),
                    image,
                    egui::TextureOptions::NEAREST,
                )
            })
            .clone();

        let size = tex.size_vec2();
        let (w, h) = (size.x.max(1.0), size.y.max(1.0));
        let scale = target_width / w;
        let display = egui::vec2(target_width, h * scale);
        ui.image((tex.id(), display));
    }
}

impl eframe::App for EscPosViewer {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Atajo rápido: alternar modo Preview/Completo.
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.ui_mode = match self.ui_mode {
                UiMode::Preview => UiMode::Full,
                UiMode::Full => UiMode::Preview,
            };
        }

        // Cachear HWND (Windows) lo antes posible.
        self.window.try_update_from_frame(frame);

        // Al entrar a modo Preview: mover la ventana cerca del borde derecho con un pequeño margen.
        if self.ui_mode == UiMode::Preview && self.last_ui_mode != UiMode::Preview {
            #[cfg(target_os = "windows")]
            {
                self.window.snap_near_right(14);
            }
        }

        // Aplicar tamaño inicial una sola vez.
        if !self.did_apply_initial_window_size {
            self.did_apply_initial_window_size = true;
            let w = Self::target_window_width_px(self.paper_width);
            Self::request_window_width(ctx, w);
        }

        // Si arrancamos en Preview, centrar la ventana una sola vez.
        if !self.did_apply_initial_window_position {
            self.did_apply_initial_window_position = true;
            if self.ui_mode == UiMode::Preview {
                #[cfg(target_os = "windows")]
                {
                    self.window.center_on_screen();
                }
            }
        }

        // Si cambió el papel, ajustar ancho de ventana.
        if self.paper_width != self.last_paper_width {
            self.last_paper_width = self.paper_width;
            let w = Self::target_window_width_px(self.paper_width);
            Self::request_window_width(ctx, w);
        }

        // Inicializar System Tray una sola vez.
        if self.tray.is_none() && self.tray_error.is_none() {
            match SystemTray::new(self.window.clone()) {
                Ok(tray) => self.tray = Some(tray),
                Err(e) => self.tray_error = Some(e),
            }
        }

        // Si el usuario intenta cerrar la ventana (X), ocultamos a bandeja.
        // Nota: esto solo aplica si el tray existe; si falló, dejamos que cierre normal.
        if self.tray.is_some() {
            let close_requested = ctx.input(|i| i.viewport().close_requested());
            if close_requested {
                self.pending_hide_to_tray = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        // Si se pidió ocultar a bandeja (por cerrar ventana), lo aplicamos aquí.
        if self.pending_hide_to_tray {
            self.pending_hide_to_tray = false;
            self.hidden_to_tray = true;
            self.hide_to_tray(ctx);
        }

        self.tick_simulation();
        if self.active_job().is_some_and(|j| j.sim_active) {
            // Forzar repaints para animar la simulación.
            ctx.request_repaint();
        }

        // Autolimpieza / límites del historial.
        self.prune_jobs();

        // Mantener el listener TCP 9100 sincronizado con el checkbox.
        // Evita reintentos automáticos constantes si el puerto está ocupado.
        if self.tcp_enabled {
            if self.tcp_capture.is_none() && self.tcp_last_error.is_none() {
                self.set_tcp_capture(true, Some(ctx.clone()));
            }
        } else if self.tcp_capture.is_some() {
            self.set_tcp_capture(false, None);
        }

        // Captura TCP (impresora virtual 9100)
        if let Some(cap) = &self.tcp_capture {
            let jobs = cap.try_recv_all();
            for job in jobs {
                let label = format!("TCP 9100 ({})", job.source);
                self.push_new_job(label, job.bytes);

                // Si estaba oculto a la bandeja, el hilo TCP ya lo re-muestra (Windows).
                self.hidden_to_tray = false;
            }
        }

        // Drag & Drop
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped.first() {
                if let Some(path) = &file.path {
                    self.try_load_path(path);
                }
            }
        }

        if self.ui_mode == UiMode::Full {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                if ui.button("📂 Abrir Archivo").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Printer Files", &["prn", "bin", "txt"])
                        .pick_file()
                    {
                        self.try_load_path(&path);
                    }
                }

                ui.separator();
                egui::ComboBox::from_label("Modo")
                    .selected_text(match self.ui_mode {
                        UiMode::Preview => "Preview",
                        UiMode::Full => "Completo",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.ui_mode, UiMode::Preview, "Preview");
                        ui.selectable_value(&mut self.ui_mode, UiMode::Full, "Completo");
                    });

                ui.separator();
                let label = if self.show_debug_panels {
                    "Ocultar Hex/Log"
                } else {
                    "Mostrar Hex/Log"
                };
                if ui.button(label).clicked() {
                    self.show_debug_panels = !self.show_debug_panels;
                }

                ui.separator();
                let enabled_before = self.tcp_enabled;
                ui.checkbox(&mut self.tcp_enabled, "Escuchar impresora (TCP 9100)");
                if self.tcp_enabled != enabled_before {
                    if self.tcp_enabled {
                        // Intentar iniciar inmediatamente al activar.
                        self.set_tcp_capture(true, Some(ctx.clone()));
                    } else {
                        self.set_tcp_capture(false, None);
                    }
                }
                if let Some(err) = &self.tcp_last_error {
                    ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
                }

                ui.separator();
                let before_sim = self.simulate_printing;
                ui.checkbox(&mut self.simulate_printing, "Simular impresión");
                if before_sim && !self.simulate_printing {
                    // Si el usuario desactiva en medio de la simulación, mostramos todo.
                    self.stop_active_simulation_show_full();
                }
                ui.add(
                    egui::Slider::new(&mut self.sim_bytes_per_sec, 1_000..=200_000)
                        .text("bytes/s"),
                );
                if let Some(job) = self.active_job() {
                    if job.sim_active {
                        let total = job.full_bytes.len().max(1);
                        let pct = (job.sim_sent as f32 / total as f32) * 100.0;
                    ui.label(egui::RichText::new(format!("{pct:.0}%"))
                        .color(egui::Color32::DARK_GRAY)
                        .small());
                    }
                }

                ui.separator();
                ui.label("Papel:");
                ui.selectable_value(&mut self.paper_width, PaperWidth::W58mm, "58mm");
                ui.selectable_value(&mut self.paper_width, PaperWidth::W80mm, "80mm");

                ui.separator();
                ui.checkbox(&mut self.show_debug_controls, "Debug comandos");

                ui.separator();
                let before = self.codepage;
                egui::ComboBox::from_label("Codepage")
                    .selected_text(match self.codepage {
                        CodePage::Utf8Lossy => "UTF-8 (lossy)",
                        CodePage::Cp437 => "CP437",
                        CodePage::Cp850 => "CP850",
                        CodePage::Windows1252 => "Windows-1252",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.codepage, CodePage::Utf8Lossy, "UTF-8 (lossy)");
                        ui.selectable_value(&mut self.codepage, CodePage::Cp437, "CP437");
                        ui.selectable_value(&mut self.codepage, CodePage::Cp850, "CP850");
                        ui.selectable_value(&mut self.codepage, CodePage::Windows1252, "Windows-1252");
                    });
                if self.codepage != before {
                    self.reparse_all_jobs();
                }

                if let Some(job) = self.active_job() {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("📄 {}", job.label)).weak());
                }
                });

                // Barra de jobs (historial / pestañas)
                self.ui_job_tabs(ui);
            });
        }

        if self.ui_mode == UiMode::Full && self.show_debug_panels {
            egui::SidePanel::left("debug_panels")
                .resizable(true)
                .min_width(260.0)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.heading("Hex / Log");
                    ui.separator();

                    egui::CollapsingHeader::new("Hex Dump")
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("hex_scroll")
                                .show(ui, |ui| {
                                    if let Some(job) = self.active_job() {
                                        ui.monospace(pretty_hex(&job.display_bytes));
                                    } else {
                                        ui.monospace("(sin datos)");
                                    }
                                });
                        });

                    ui.add_space(8.0);

                    egui::CollapsingHeader::new("Log (Comandos)")
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("cmd_scroll")
                                .show(ui, |ui| {
                                    let Some(job) = self.active_job() else {
                                        ui.label(egui::RichText::new("(sin comandos)").weak());
                                        return;
                                    };
                                    for (idx, (_state, cmd)) in
                                        job.parsed_commands.iter().enumerate()
                                    {
                                        let line = match cmd {
                                            CommandType::Text(text) => {
                                                let mut snippet =
                                                    text.replace(['\r', '\n'], " ");
                                                const MAX: usize = 60;
                                                if snippet.len() > MAX {
                                                    snippet.truncate(MAX);
                                                    snippet.push_str("…");
                                                }
                                                format!("TXT  {}", snippet)
                                            }
                                            CommandType::Control(control) => format!(
                                                "CTL  {}",
                                                Self::debug_label_for_control(control)
                                            ),
                                            CommandType::Unknown(byte) => {
                                                format!("UNK  {:02X}", byte)
                                            }
                                        };

                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{:04}: {}",
                                                idx, line
                                            ))
                                            .monospace()
                                            .size(10.0),
                                        );
                                    }
                                });
                        });
                });
        }

        // En modo Preview: botón flotante para volver a mostrar menús.
        if self.ui_mode == UiMode::Preview {
            egui::Area::new("preview_menu_button".into())
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 10.0))
                .interactable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Menú").clicked() {
                            self.ui_mode = UiMode::Full;
                        }
                        ui.label(egui::RichText::new("(F1)").weak().small());
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let (job_id, stick_bottom) = match self.active_job() {
                Some(j) => (j.id, self.auto_scroll_on_print && j.sim_active),
                None => (0, false),
            };

            ui.push_id(job_id, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("render_scroll")
                    .stick_to_bottom(stick_bottom)
                    .show(ui, |ui| {
                    let desired: f32 = match self.paper_width {
                        PaperWidth::W58mm => 300.0,
                        PaperWidth::W80mm => 450.0,
                    };
                    let available: f32 = ui.available_width().max(0.0);
                    let paper_width: f32 = desired.min((available - 20.0).max(180.0));

                    ui.vertical_centered(|ui| {
                        let ticket = egui::Frame::none()
                            .fill(egui::Color32::WHITE)
                            .shadow(egui::Shadow::default())
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
                            .inner_margin(15.0)
                            .rounding(2.0)
                            .show(ui, |ui| {
                                ui.set_min_width(paper_width);
                                ui.set_max_width(paper_width);
                                ui.set_min_height(400.0);

                                let mut texture_cache = mem::take(&mut self.texture_cache);

                                let Some(job) = self.active_job() else {
                                    ui.label(
                                        egui::RichText::new("Arrastra un .prn/.bin o imprime por TCP 9100")
                                            .color(egui::Color32::GRAY)
                                            .size(12.0),
                                    );
                                    self.texture_cache = texture_cache;
                                    return;
                                };

                                let mut pending: Option<(PrinterState, String)> = None;
                                let flush_pending = |ui: &mut egui::Ui,
                                                     pending: &mut Option<(PrinterState, String)>| {
                                    if let Some((s, t)) = pending.take() {
                                        if !t.is_empty() {
                                            Self::emit_text_with_columns(
                                                ui,
                                                self.paper_width,
                                                &s,
                                                &t,
                                            );
                                        }
                                    }
                                };

                                for (state, cmd) in &job.parsed_commands {
                                    match cmd {
                                        CommandType::Text(text) => match &mut pending {
                                            Some((ps, buf))
                                                if Self::same_line_style(ps, state) =>
                                            {
                                                buf.push_str(text);
                                            }
                                            Some(_) => {
                                                flush_pending(ui, &mut pending);
                                                pending =
                                                    Some((state.clone(), text.clone()));
                                            }
                                            None => {
                                                pending =
                                                    Some((state.clone(), text.clone()));
                                            }
                                        },
                                        CommandType::Control(control) => {
                                            if self.show_debug_controls {
                                                let label =
                                                    Self::debug_label_for_control(control);
                                                ui.label(
                                                    egui::RichText::new(label)
                                                        .size(9.0)
                                                        .color(egui::Color32::GRAY)
                                                        .monospace(),
                                                );
                                            }

                                            match control {
                                                Control::Newline => {
                                                    flush_pending(ui, &mut pending);
                                                    ui.add_space(5.0);
                                                }
                                                Control::Cut => {
                                                    flush_pending(ui, &mut pending);
                                                    ui.add_space(15.0);
                                                    ui.label(
                                                        egui::RichText::new(
                                                            "- - - - - - CORTE - - - - - -",
                                                        )
                                                        .size(10.0)
                                                        .color(egui::Color32::GRAY),
                                                    );
                                                    ui.add_space(15.0);
                                                }
                                                Control::RasterImage {
                                                    m: _,
                                                    width_bytes,
                                                    height,
                                                    data,
                                                } => {
                                                    flush_pending(ui, &mut pending);
                                                    if let Some(img) = Self::raster_to_image(
                                                        *width_bytes,
                                                        *height,
                                                        data,
                                                    ) {
                                                        let key = Self::hash_key(&(
                                                            "raster",
                                                            width_bytes,
                                                            height,
                                                            data,
                                                        ));
                                                        Self::show_image_scaled(
                                                            ui,
                                                            &mut texture_cache,
                                                            key,
                                                            img,
                                                            paper_width,
                                                        );
                                                        ui.add_space(8.0);
                                                    }
                                                }
                                                Control::Qr {
                                                    model: _,
                                                    module_size,
                                                    ecc,
                                                    data,
                                                } => {
                                                    flush_pending(ui, &mut pending);
                                                    if let Some(img) = Self::qr_to_image(
                                                        data,
                                                        *ecc,
                                                        *module_size,
                                                    ) {
                                                        let key = Self::hash_key(&(
                                                            "qr",
                                                            ecc,
                                                            module_size,
                                                            data,
                                                        ));
                                                        let target =
                                                            paper_width.min(260.0);
                                                        ui.vertical_centered(|ui| {
                                                            Self::show_image_scaled(
                                                                ui,
                                                                &mut texture_cache,
                                                                key,
                                                                img,
                                                                target,
                                                            );
                                                        });
                                                        ui.add_space(8.0);
                                                    } else {
                                                        ui.label(
                                                            egui::RichText::new(
                                                                "[QR inválido]",
                                                            )
                                                            .color(egui::Color32::GRAY)
                                                            .monospace(),
                                                        );
                                                    }
                                                }
                                                Control::Barcode { m, data } => {
                                                    flush_pending(ui, &mut pending);
                                                    let preview =
                                                        String::from_utf8_lossy(data);
                                                    ui.add_space(6.0);
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "[BARCODE m={:02X}] {}",
                                                            m, preview
                                                        ))
                                                        .color(egui::Color32::BLACK)
                                                        .monospace()
                                                        .size(11.0),
                                                    );
                                                    ui.add_space(6.0);
                                                }
                                                _ => {}
                                            }
                                        }
                                        CommandType::Unknown(_) => {}
                                    }
                                }

                                flush_pending(ui, &mut pending);

                                self.texture_cache = texture_cache;
                            });

                        if let Some(job) = self.active_job() {
                            if job.sim_active && !job.full_bytes.is_empty() {
                                let progress = job.sim_sent as f32 / job.full_bytes.len() as f32;
                            Self::draw_printing_reveal_effect(ui, ticket.response.rect, progress);
                            }
                        }

                        if self.ui_mode == UiMode::Preview {
                            ticket.response.context_menu(|ui| {
                                ui.label("Modo");
                                ui.separator();
                                ui.selectable_value(&mut self.ui_mode, UiMode::Preview, "Preview");
                                ui.selectable_value(&mut self.ui_mode, UiMode::Full, "Completo");
                            });
                        }
                    });
                });
            });
        });

            self.last_ui_mode = self.ui_mode;
    }
}
