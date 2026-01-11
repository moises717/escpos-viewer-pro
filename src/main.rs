mod app;
mod app_icon;
mod escpos;
mod hex_dump;
mod model;
mod printer_setup;
mod tcp_capture;
mod tray;
mod window_control;

fn main() -> eframe::Result<()> {
    // Modo instalador/CLI (Windows): permite que un instalador cree la impresora virtual.
    // Requiere ejecutar como Administrador.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--install-printer") {
        match printer_setup::install_printer() {
            Ok(()) => {
                println!("OK: impresora instalada");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(1);
            }
        }
    }
    if args.iter().any(|a| a == "--uninstall-printer") {
        match printer_setup::uninstall_printer() {
            Ok(()) => {
                println!("OK: impresora desinstalada");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(1);
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([480.0, 600.0])
            .with_title("Visor ESC-POS")
            .with_icon(app_icon::eframe_icon_data().unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Visor ESC/POS",
        options,
        Box::new(|_cc| Ok(Box::new(app::EscPosViewer::default()))),
    )
}