use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use padmap_core::{Mapper, Profile};
use padmap_input::{GamepadSource, SourceEvent};
use padmap_output::{platform_sink, DryRunSink, KeyboardSink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "padmap",
    version,
    about = "Mapeia qualquer controle para o teclado"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lista os controles conectados.
    List,
    /// Mostra os eventos do controle, para descobrir os nomes dos botões.
    Monitor,
    /// Valida um perfil e mostra como ele foi interpretado.
    Check { profile: PathBuf },
    /// Lê o controle e envia teclas de acordo com o perfil.
    Run {
        profile: PathBuf,
        /// Não injeta teclas, só mostra o que seria enviado.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    match Cli::parse().command {
        Command::List => list(),
        Command::Monitor => monitor(),
        Command::Check { profile } => check(&profile),
        Command::Run { profile, dry_run } => run(&profile, dry_run),
    }
}

fn open_source() -> Result<GamepadSource> {
    GamepadSource::new()
        .map_err(anyhow::Error::msg)
        .context("falha ao inicializar o SDL")
}

fn list() -> Result<()> {
    let mut source = open_source()?;
    // O SDL anuncia os controles já conectados como eventos de "added".
    while source.next_event(Duration::from_millis(200)).is_some() {}
    let pads = source.connected();
    if pads.is_empty() {
        println!("Nenhum controle encontrado.");
    }
    for (id, name) in pads {
        println!("{id}: {name}");
    }
    Ok(())
}

fn stop_flag() -> Result<Arc<AtomicBool>> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))
        .context("falha ao registrar Ctrl+C")?;
    Ok(running)
}

fn monitor() -> Result<()> {
    let mut source = open_source()?;
    let running = stop_flag()?;
    println!("Mexa no controle (Ctrl+C para sair).");
    while running.load(Ordering::SeqCst) {
        match source.next_event(Duration::from_millis(100)) {
            Some(SourceEvent::Quit) => break,
            Some(e) => println!("{e:?}"),
            None => {}
        }
    }
    Ok(())
}

fn check(path: &PathBuf) -> Result<()> {
    let profile = Profile::load(path)?;
    println!("Perfil \"{}\" OK.", profile.name);
    for (button, combo) in &profile.buttons {
        println!("  {button:<16} -> {combo}");
    }
    for (axis, b) in &profile.axes {
        let fmt = |c: &Option<_>| {
            c.as_ref()
                .map_or("-".to_string(), |c: &padmap_core::KeyCombo| c.to_string())
        };
        println!(
            "  {axis:<16} -: {}  +: {}",
            fmt(&b.negative),
            fmt(&b.positive)
        );
    }
    Ok(())
}

fn run(path: &PathBuf, dry_run: bool) -> Result<()> {
    let profile = Profile::load(path)?;
    log::info!("perfil carregado: {}", profile.name);
    let mut mapper = Mapper::new(profile);
    let mut sink: Box<dyn KeyboardSink> = if dry_run {
        Box::new(DryRunSink)
    } else {
        platform_sink()?
    };
    let mut source = open_source()?;
    let running = stop_flag()?;
    // Por enquanto todos os controles conectados compartilham o mesmo perfil.
    // Perfis por controle entram quando houver a interface gráfica.
    log::info!("rodando; Ctrl+C para sair");
    while running.load(Ordering::SeqCst) {
        match source.next_event(Duration::from_millis(100)) {
            Some(SourceEvent::Connected { id, name }) => {
                log::info!("controle {id} conectado: {name}")
            }
            Some(SourceEvent::Disconnected { id }) => {
                log::info!("controle {id} desconectado");
                sink.send_all(&mapper.release_all())?;
            }
            Some(SourceEvent::Input { event, .. }) => sink.send_all(&mapper.handle(event))?,
            Some(SourceEvent::Quit) => break,
            None => {}
        }
    }
    sink.send_all(&mapper.release_all())?;
    log::info!("encerrado");
    Ok(())
}
