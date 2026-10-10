# Earth — Kendi Dünyan, Kendi Kodlaman

Earth is a small, multilingual programming language experiment. The first
working MVP focuses on one useful capability: printing quoted text from a
source file.

## Current scope

The MVP supports these equivalent statements:

```earth
yaz("Merhaba, Earth!")
print("Hello, Earth!")
```

Statements may be separated by whitespace or newlines. Strings use double
quotes and support the escapes `\"`, `\\`, `\n`, `\r`, and `\t`.

## Setup and usage

Install [Rust](https://www.rust-lang.org/tools/install), then run a source
file with Cargo:

```bash
cargo run -- examples/hello.earth
```

You can also build and run the binary directly:

```bash
cargo build
target/debug/earth examples/hello.earth
```

Run the checks:

```bash
cargo fmt -- --check
cargo test
```

On Windows, the default Rust target is `x86_64-pc-windows-msvc` and requires
the MSVC linker. If Cargo reports `link.exe not found`, install **Visual
Studio Build Tools** with the **Desktop development with C++** workload
(including the Windows 10/11 SDK), then open **x64 Native Tools Command
Prompt for VS** and run:

```bat
where cl
where link
cargo test
cargo run -- examples\hello.earth
```

The checks above should print paths under `VC\Tools\MSVC` and the program
should print both greeting lines. A `link.exe` found only under an SDK helper
directory is not sufficient; the C++ workload must provide the matching MSVC
compiler, libraries, and linker environment.

## Architecture

- **Lexer** (`src/lexer.rs`) converts source text into tokens and reports
  unterminated or invalid strings.
- **Parser and AST** (`src/parser.rs`, `src/ast.rs`) validate function-call
  syntax and create a small typed representation.
- **Interpreter** (`src/interpreter.rs`) evaluates print statements and
  returns their output.
- **CLI** (`src/main.rs`) reads a `.earth` source file, runs the pipeline, and
  presents useful errors.

The implementation uses only Rust's standard library to stay quick to build
and suitable for machines with limited memory.

## Yeni bir built-in komut eklemek

Yeni bir komut veya alias eklemek, dilin katmanlarını sırayla genişletmek için
bilinçli olarak küçük tutulmuştur:

1. `src/ast.rs` içine komutun taşıdığı veriyi temsil eden yeni bir `Statement`
   varyantı ekleyin.
2. `src/parser.rs` içinde komut adını ve parantez içindeki sözdizimini
   doğrulayın; Türkçe ve İngilizce adları aynı AST varyantına yönlendirin.
3. `src/interpreter.rs` içinde yeni AST varyantının davranışını uygulayın.
4. `src/lib.rs` veya ilgili modüle hem alias hem de hatalı sözdizimi için
   küçük unit testler ekleyin.
5. `README.md` içindeki **Current scope** örneklerini ve bu rehberi güncel
   tutun.

Lexer yalnızca yeni semboller veya string kuralları gerektiğinde (`src/lexer.rs`)
değiştirilmelidir. CLI (`src/main.rs`) komutları tek tek tanımamalıdır; kaynak
dosyasını pipeline'a bırakmalıdır. Bu ayrım, yeni komutları öğrenirken her
katmanın sorumluluğunu izlemeyi kolaylaştırır.

## Roadmap

1. Variables and basic values.
2. Expressions and arithmetic.
3. Conditions and loops.
4. A richer Turkish/English keyword system.
5. Diagnostics with source locations and an interactive REPL.
