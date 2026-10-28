# O SDL é compilado pelo CMake, que ignora o `crt-static` do Rust: sem isto ele
# usa o runtime dinâmico (/MD) e o link do padmap.exe falha.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
