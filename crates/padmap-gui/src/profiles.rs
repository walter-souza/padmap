//! Biblioteca de perfis: a pasta onde o padmap guarda os perfis do usuário.
//! A interface lista o que está nela, e salvar, criar, duplicar e importar
//! gravam ali sem perguntar onde.

use crate::capture::combo_label;
use crate::slots::stick_short;
use padmap_core::{Profile, StickMode};
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = include_str!("../../../profiles/default.toml");

/// Um perfil da pasta, como aparece na lista.
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub summary: String,
}

pub struct Library {
    dir: PathBuf,
}

impl Library {
    /// A pasta `perfis` nos dados do padmap (no Windows,
    /// `%APPDATA%\padmap\data\perfis`). Na primeira vez, cria o perfil padrão.
    pub fn open() -> Result<Library, String> {
        let dir = eframe::storage_dir("padmap")
            .ok_or("não foi possível achar a pasta de dados do usuário")?
            .join("perfis");
        Library::at(dir)
    }

    pub fn at(dir: PathBuf) -> Result<Library, String> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("não foi possível criar {}: {e}", dir.display()))?;
        let lib = Library { dir };
        lib.ensure_not_empty()?;
        Ok(lib)
    }

    /// Com a pasta vazia, recria o perfil padrão: sempre há um para abrir.
    pub fn ensure_not_empty(&self) -> Result<(), String> {
        if self.list().is_empty() {
            let default = Profile::from_toml(DEFAULT_PROFILE).expect("perfil padrão válido");
            self.create(&default)?;
        }
        Ok(())
    }

    /// Perfis da pasta em ordem alfabética. Arquivos que não são perfis
    /// válidos ficam de fora (e vão para o log).
    pub fn list(&self) -> Vec<Entry> {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut entries: Vec<Entry> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .filter_map(|path| match Profile::load(&path) {
                Ok(p) => Some(Entry {
                    summary: summary(&p),
                    name: p.name,
                    path,
                }),
                Err(e) => {
                    log::warn!("perfil ignorado: {e}");
                    None
                }
            })
            .collect();
        entries.sort_by_key(|e| e.name.to_lowercase());
        entries
    }

    /// Grava um perfil novo na pasta, com um nome de arquivo ainda livre.
    pub fn create(&self, profile: &Profile) -> Result<PathBuf, String> {
        let path = free_path(&self.dir, &profile.name);
        std::fs::write(&path, profile.to_toml())
            .map_err(|e| format!("Não foi possível criar o perfil: {e}"))?;
        Ok(path)
    }

    /// Cópia do perfil em `path`, chamada "<nome> (cópia)".
    pub fn duplicate(&self, path: &Path) -> Result<PathBuf, String> {
        let mut p = Profile::load(path).map_err(|e| e.to_string())?;
        p.name = format!("{} (cópia)", p.name);
        self.create(&p)
    }

    /// Traz para a pasta um perfil de fora.
    pub fn import(&self, src: &Path) -> Result<PathBuf, String> {
        let p = Profile::load(src).map_err(|e| e.to_string())?;
        self.create(&p)
    }
}

/// `<nome>.toml` na pasta, ou `<nome> 2.toml`, `<nome> 3.toml`… se já existir.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let stem = file_stem(name);
    let mut path = dir.join(format!("{stem}.toml"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} {n}.toml"));
        n += 1;
    }
    path
}

/// Troca o nome gravado no arquivo do perfil (e só ele) e renomeia o arquivo
/// para `<nome>.toml` na mesma pasta. Devolve o novo caminho.
pub fn rename(path: &Path, new: &str) -> Result<PathBuf, String> {
    let mut on_disk = Profile::load(path).map_err(|e| e.to_string())?;
    on_disk.name = new.to_owned();
    let target = path.with_file_name(format!("{}.toml", file_stem(new)));
    // No Windows "Perfil.toml" e "perfil.toml" são o mesmo arquivo.
    let same = match (std::fs::canonicalize(path), std::fs::canonicalize(&target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same && target.exists() {
        return Err(format!(
            "Já existe um perfil chamado \u{201c}{new}\u{201d}."
        ));
    }
    std::fs::write(path, on_disk.to_toml())
        .map_err(|e| format!("Não foi possível renomear: {e}"))?;
    if target != path {
        std::fs::rename(path, &target).map_err(|e| format!("Não foi possível renomear: {e}"))?;
    }
    Ok(target)
}

/// Manda o arquivo do perfil para a lixeira.
pub fn delete(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("Não foi possível excluir: {e}"))
}

/// Nome de arquivo a partir do nome do perfil, sem os caracteres que o Windows
/// não aceita.
fn file_stem(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c => c,
        })
        .collect()
}

/// Resumo de uma linha do que o perfil faz, para a lista
/// (ex.: "12 botões · LS: W A S D · RS: mouse").
pub fn summary(p: &Profile) -> String {
    let buttons = p.buttons.len()
        + p.axes
            .values()
            .filter(|a| a.positive.is_some() || a.negative.is_some())
            .count()
        + p.extras.iter().filter(|e| e.key.is_some()).count();
    let mut parts = vec![match buttons {
        0 => "nenhum botão".to_owned(),
        1 => "1 botão".to_owned(),
        n => format!("{n} botões"),
    }];
    for (&stick, b) in &p.sticks {
        let what = match b.mode {
            StickMode::Mouse => "mouse".to_owned(),
            StickMode::Sectors => {
                let keys: Vec<String> = b
                    .sectors
                    .iter()
                    .filter_map(|s| s.key.as_ref().map(combo_label))
                    .collect();
                if keys.is_empty() {
                    continue;
                }
                keys.join(" ")
            }
        };
        parts.push(format!("{}: {what}", stick_short(stick)));
    }
    if let Some(tp) = &p.touchpad {
        let keys: Vec<String> = tp
            .zones
            .iter()
            .filter_map(|z| z.key.as_ref().map(combo_label))
            .collect();
        parts.push(format!("TP: {}", keys.join(" ")));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pasta temporária só deste teste, apagada no fim.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let dir = std::env::temp_dir().join(format!("padmap-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn new_library_gets_the_default_profile() {
        let tmp = TempDir::new("lib-default");
        let lib = Library::at(tmp.0.clone()).unwrap();
        let list = lib.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Padrão (WASD)");
        assert!(
            list[0].summary.contains("LS: W A S D"),
            "{}",
            list[0].summary
        );
    }

    #[test]
    fn create_duplicate_and_import_never_overwrite() {
        let tmp = TempDir::new("lib-create");
        let lib = Library::at(tmp.0.clone()).unwrap();
        let first = lib.list().remove(0).path;
        let copy = lib.duplicate(&first).unwrap();
        assert_eq!(Profile::load(&copy).unwrap().name, "Padrão (WASD) (cópia)");
        let again = lib.import(&first).unwrap();
        assert_ne!(again, first);
        assert!(again.ends_with("Padrão (WASD) 2.toml"));
        assert_eq!(lib.list().len(), 3);
    }

    #[test]
    fn invalid_files_are_left_out() {
        let tmp = TempDir::new("lib-invalid");
        let lib = Library::at(tmp.0.clone()).unwrap();
        std::fs::write(tmp.0.join("quebrado.toml"), "isso não é toml =").unwrap();
        std::fs::write(tmp.0.join("notas.txt"), "x").unwrap();
        assert_eq!(lib.list().len(), 1);
    }

    #[test]
    fn rename_moves_file_and_keeps_the_rest() {
        let tmp = TempDir::new("lib-rename");
        std::fs::create_dir_all(&tmp.0).unwrap();
        let old = tmp.0.join("Velho.toml");
        std::fs::write(&old, "name = \"Velho\"\n[buttons]\nsouth = \"Space\"\n").unwrap();

        let new = rename(&old, "Novo").unwrap();
        assert_eq!(new, tmp.0.join("Novo.toml"));
        assert!(!old.exists());
        let p = Profile::load(&new).unwrap();
        assert_eq!(p.name, "Novo");
        assert_eq!(p.buttons.len(), 1);

        // Só maiúsculas: no Windows é o mesmo arquivo e não pode sumir.
        let same = rename(&new, "NOVO").unwrap();
        assert!(same.exists());
        assert_eq!(Profile::load(&same).unwrap().name, "NOVO");

        // Não sobrescreve outro perfil.
        std::fs::write(tmp.0.join("Outro.toml"), "name = \"Outro\"\n").unwrap();
        assert!(rename(&same, "Outro").is_err());
        assert!(same.exists());
    }

    #[test]
    fn file_stem_drops_forbidden_chars() {
        assert_eq!(file_stem("FPS: mira/tiro?"), "FPS_ mira_tiro_");
        assert_eq!(file_stem("Padrão (WASD)"), "Padrão (WASD)");
    }
}
