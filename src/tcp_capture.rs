use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use eframe::egui;
use crate::window_control::WindowControl;

#[derive(Debug, Clone)]
pub struct CapturedJob {
    pub source: String,
    pub bytes: Vec<u8>,
}

pub struct TcpCapture {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    rx: Receiver<CapturedJob>,
}

impl TcpCapture {
    pub fn start(
        bind_addr: &str,
        repaint_ctx: Option<egui::Context>,
        window: Option<WindowControl>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;

        let (tx, rx) = mpsc::channel::<CapturedJob>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let bind_addr_string = bind_addr.to_string();

        let join = thread::spawn(move || {
            loop {
                if stop_thread.load(Ordering::Relaxed) {
                    break;
                }

                match listener.accept() {
                    Ok((stream, peer)) => {
                        let tx = tx.clone();
                        let source = format!("{} -> {}", peer, bind_addr_string);
                        if let Err(err) =
                            read_one_job(stream, source, tx, repaint_ctx.clone(), window.clone())
                        {
                            let _ = err; // silencioso
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => {
                        // Si el accept falla por otra cosa, salimos para evitar loop caliente.
                        break;
                    }
                }
            }
        });

        Ok(Self {
            stop,
            join: Some(join),
            rx,
        })
    }

    pub fn try_recv_all(&self) -> Vec<CapturedJob> {
        self.rx.try_iter().collect()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for TcpCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn read_one_job(
    mut stream: TcpStream,
    source: String,
    tx: Sender<CapturedJob>,
    repaint_ctx: Option<egui::Context>,
    window: Option<WindowControl>,
) -> std::io::Result<()> {
    // Normalmente Windows abre conexin, manda bytes y cierra (EOF) por job.
    // Pongo timeout por si el peer se queda abierto.
    // Un timeout muy corto puede partir un ticket en 2 jobs si el POS manda en ráfagas.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];

    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Consideramos fin de job por inactividad.
                break;
            }
            Err(e) => return Err(e),
        }
    }

    if !buf.is_empty() {
        let _ = tx.send(CapturedJob { source, bytes: buf });
        if let Some(w) = window {
            w.show_and_focus();
        }
        if let Some(ctx) = repaint_ctx {
            ctx.request_repaint();
        }
    }

    Ok(())
}
