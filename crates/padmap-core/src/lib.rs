//! Núcleo do padmap: tipos de entrada de controle, teclas, perfis e o motor de
//! mapeamento. Não depende de SDL nem de APIs de sistema operacional, então tudo
//! aqui é testável em qualquer plataforma.

pub mod gamepad;
pub mod key;
pub mod mapper;
pub mod profile;

pub use gamepad::{Axis, Button, InputEvent};
pub use key::{Key, KeyCombo};
pub use mapper::{KeyAction, Mapper};
pub use profile::{AxisBinding, Profile, ProfileError, Settings};
