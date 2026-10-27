//! Saída de teclado e mouse. Cada sistema operacional tem seu backend,
//! escolhido em tempo de compilação por `platform_sink()`; o resto do app só
//! conhece o trait `OutputSink`.

mod codes;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use padmap_core::KeyAction;

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("falha ao inicializar a saída de teclado: {0}")]
    Init(String),
    #[error("falha ao enviar tecla: {0}")]
    Send(String),
    #[error("injeção de teclado ainda não suportada neste sistema")]
    Unsupported,
}

pub trait OutputSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError>;

    /// Move o cursor `(dx, dy)` pixels a partir de onde ele está, com y para
    /// baixo. Movimento relativo, como um mouse de verdade, para funcionar
    /// também em jogos que prendem o cursor.
    fn move_mouse(&mut self, dx: i32, dy: i32) -> Result<(), OutputError>;

    fn send_all(&mut self, actions: &[KeyAction]) -> Result<(), OutputError> {
        actions.iter().try_for_each(|&a| self.send(a))
    }
}

/// Backend nativo do sistema atual.
pub fn platform_sink() -> Result<Box<dyn OutputSink>, OutputError> {
    #[cfg(windows)]
    return Ok(Box::new(windows::SendInputSink::new()));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(linux::UinputSink::new()?));
    #[cfg(target_os = "macos")]
    return Ok(Box::new(macos::CgEventSink::new()?));
    #[allow(unreachable_code)]
    Err(OutputError::Unsupported)
}

/// Só registra as teclas no log, sem injetar nada. Útil para testar perfis.
#[derive(Default)]
pub struct DryRunSink;

impl OutputSink for DryRunSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        match action {
            KeyAction::Press(k) => log::info!("[simulação] pressiona {k}"),
            KeyAction::Release(k) => log::info!("[simulação] solta {k}"),
        }
        Ok(())
    }

    /// Não registra nada: o mouse anda a cada poucos milissegundos e
    /// inundaria o log.
    fn move_mouse(&mut self, _dx: i32, _dy: i32) -> Result<(), OutputError> {
        Ok(())
    }
}
