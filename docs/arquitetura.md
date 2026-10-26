# Arquitetura

## Stack

| Peça | Escolha | Por quê |
|---|---|---|
| Linguagem | Rust | binário único e sem runtime, acesso direto às APIs nativas, compila para os três sistemas |
| Entrada | SDL3 (crate `sdl3`, compilado estático) | base de mapeamentos `gamecontrollerdb`, drivers HIDAPI para DualShock 4/DualSense/Switch Pro, XInput e Windows.Gaming.Input no Windows. Um só código para qualquer controle |
| Saída | uma implementação por SO | não existe API portátil para injetar teclas |
| Perfis | TOML (`serde`) | legível e editável à mão |

## Crates

```
crates/
  padmap-core    tipos (Button, Axis, Key), perfis TOML e o Mapper. Sem SDL nem SO; 100% testável.
  padmap-input   GamepadSource: SDL3 -> InputEvent normalizado.
  padmap-output  trait KeyboardSink + backends:
                   windows.rs  SendInput com scancodes (KEYEVENTF_SCANCODE)
                   linux.rs    teclado virtual via /dev/uinput (X11 e Wayland)
                   macos.rs    CGEventPost (exige permissão de Acessibilidade)
                 codes.rs tem as três tabelas Key -> código nativo.
  padmap         binário de linha de comando (list, monitor, check, run).
```

Fluxo: `GamepadSource::next_event` -> `Mapper::handle` -> `Vec<KeyAction>` -> `KeyboardSink::send`.

## Decisões

- **Layout posicional.** O perfil fala em `south/east/west/north`, não em
  A/B/X/Y ou Cruz/Círculo. Assim o mesmo perfil vale para Xbox, PS e Nintendo.
- **Scancodes no Windows.** Jogos que leem DirectInput/Raw Input ignoram
  teclas virtuais (`wVk`); scancodes funcionam em jogos e em apps comuns.
- **Contagem de referência.** Dois botões ligados à mesma tecla só a soltam
  quando ambos forem soltos.
- **Histerese nos eixos.** Pressiona em `threshold`, solta em
  `threshold * release_ratio`.
- **Eventos em segundo plano.** O hint `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS`
  é ligado porque o foco está sempre no jogo, não no padmap.
- **SDL enxuto.** O SDL é compilado sem vídeo, áudio, render e GPU; só
  joystick, gamepad, HIDAPI e eventos.

## Próximos passos

1. Testar em Windows com controles reais (Xbox, DualSense, genérico).
2. Ícone na bandeja e troca de perfil por atalho no controle.
3. Interface gráfica para editar perfis (candidatos: `egui` ou Tauri).
4. Perfis por controle e por jogo (detectar o processo em primeiro plano).
5. Mapear para mouse (stick como cursor), turbo, macros e camadas (segurar um botão muda o mapa).
6. Instalador Windows (MSI/winget) e pacotes Linux/macOS.
