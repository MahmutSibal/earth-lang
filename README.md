# Earth — Kendi Dünyan, Kendi Kodlaman

Earth, anahtar kelimeleri birden fazla insan dilinde yazılabilen küçük bir
programlama dilidir. Şu an Türkçe ve İngilizce destekleniyor ve ikisi aynı
programda bile karıştırılabilir.

*Earth is a small multilingual programming language: every keyword and
built-in has Turkish and English spellings that mean exactly the same thing.*

```earth
# FizzBuzz
her sayı içinde aralık(1, 16) {
    eğer sayı % 15 == 0 {
        yaz("FizzBuzz")
    } değilse eğer sayı % 3 == 0 {
        yaz("Fizz")
    } değilse {
        yaz(sayı)
    }
}
```

```earth
# The same language in English
function fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
print(fib(20))
```

## Kurulum ve kullanım

[Rust](https://www.rust-lang.org/tools/install) kurun, sonra:

```bash
cargo run -- examples/fizzbuzz.earth   # bir dosya çalıştır
cargo run                              # etkileşimli kabuk (REPL)
```

Derlenmiş ikili dosyayı doğrudan da kullanabilirsiniz:

```bash
cargo build --release
target/release/earth examples/liste.earth
```

Kontroller:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Dil rehberi

### Anahtar kelimeler

Her satırdaki yazımlar birbirinin aynısıdır. Türkçe karakterleri olmayan
klavyeler için ASCII yazımlar da kabul edilir (`eger`, `degilse`, …).

| Türkçe | ASCII | English | Anlamı |
|---|---|---|---|
| `değişken` | `degisken` | `let` | değişken tanımlama |
| `eğer` | `eger` | `if` | koşul |
| `değilse` | `degilse` | `else` | aksi halde |
| `iken` | | `while` | koşul doğru iken döngü |
| `her … içinde` | `icinde` | `for … in` | liste/metin üzerinde döngü |
| `fonksiyon` | | `function`, `fn` | fonksiyon tanımlama |
| `döndür` | `dondur` | `return` | fonksiyondan değer döndürme |
| `kır` | `kir` | `break` | döngüden çıkma |
| `devam` | | `continue` | sonraki tura geçme |
| `ve` / `veya` / `değil` | `degil` | `and` / `or` / `not` | mantıksal işlemler |
| `doğru` / `yanlış` | `dogru` / `yanlis` | `true` / `false` | mantıksal değerler |
| `boş` | `bos` | `nil` | değer yok |

### Değerler ve işlemler

- **Sayılar:** `42`, `3.14`. İşlemler: `+ - * / %`. Sıfıra bölme hata verir.
- **Metinler:** `"Merhaba"`. Kaçış dizileri: `\" \\ \n \r \t`. `+` ile bir metne
  herhangi bir değer eklenebilir: `"x = " + 5`.
- **Listeler:** `[1, "iki", doğru]`. İndeksleme `liste[0]`, sondan saymak için
  `liste[-1]`, atama `liste[0] = 5`. `+` iki listeyi birleştirir.
- **Karşılaştırma:** `== != < <= > >=`. Sayılar ve metinler sıralanabilir.
- **Doğruluk:** yalnızca `yanlış` ve `boş` yanlış sayılır. `0` ve `""` doğrudur.
- `ve` / `veya` kısa devre yapar: sağ taraf yalnızca gerekirse hesaplanır.

### Değişkenler ve kapsam

```earth
değişken sayaç = 0      # tanımlama
sayaç = sayaç + 1       # güncelleme (önce tanımlanmış olmalı)
```

Her `{ … }` bloğu yeni bir kapsam açar. Blok içinde tanımlanan değişkenler
dışarıdan görünmez.

### Fonksiyonlar

```earth
fonksiyon sayaç_yap() {
    değişken n = 0
    fonksiyon artır() {
        n = n + 1
        döndür n
    }
    döndür artır          # fonksiyonlar değerdir; kapanış (closure) oluşturur
}
```

Özyineleme en fazla 1000 iç içe çağrıya kadar desteklenir.

### Yerleşik fonksiyonlar

| Türkçe | English | Açıklama |
|---|---|---|
| `yaz(a, b, …)` | `print` | değerleri boşlukla ayırıp yazdırır |
| `girdi(istem?)` | `input` | bir satır okur; dosya sonunda `boş` döner |
| `uzunluk(x)` | `len` | metin veya liste uzunluğu |
| `aralık(son)`, `aralık(baş, son, adım?)` | `range` | sayı listesi üretir |
| `ekle(liste, değer)` | `push` | listenin sonuna ekler |
| `metin(x)` | `str` | değeri metne çevirir |
| `sayı(x)` | `number` | metni sayıya çevirir; olmazsa `boş` döner |
| `tür(x)` | `type` | `"sayı"`, `"metin"`, `"liste"`, … |

### Sözdizimi notları

- Her satıra bir komut yazılır. Aynı satırda birden fazla komut için `;`
  kullanın.
- `#` ile başlayan kısım satır sonuna kadar yorumdur.
- Parantez ve köşeli parantez içindeki ifadeler birden fazla satıra yayılabilir.
- `değilse`, kapanan `}` ile aynı satırda ya da bir sonraki satırda olabilir.

### Hata mesajları

Hatalar dosya, satır ve sütun bilgisiyle birlikte gösterilir:

```text
Earth hatası: program.earth:2:14: '*' işlemi 'metin' ve 'sayı' türleri arasında yapılamaz
  |
2 | yaz(x +  "a" * 2)
  |              ^
```

## Mimari

- **`src/keywords.rs`:** dilin çok dilli sözlüğü. Anahtar kelimelerin ve
  yerleşik fonksiyonların bütün yazımları buradadır.
- **`src/lexer.rs`:** kaynak metni satır/sütun bilgili token'lara ayırır.
- **`src/parser.rs`, `src/ast.rs`:** öncelik sıralı özyinelemeli iniş
  (recursive descent) ayrıştırıcı ve sözdizimi ağacı. `kır` / `döndür` gibi
  yanlış yerde kullanılan komutları çalıştırmadan önce yakalar.
- **`src/interpreter.rs`, `src/value.rs`:** ağaç gezen yorumlayıcı, değerler ve
  kapsamlar.
- **`src/main.rs`:** dosya çalıştırıcı, hata raporlama ve REPL.

Proje yalnızca Rust standart kütüphanesini kullanır.

### Windows notu

Windows'ta varsayılan Rust hedefi `x86_64-pc-windows-msvc`'dir ve MSVC
bağlayıcısını gerektirir. `link.exe not found` hatası alırsanız **Visual Studio
Build Tools** içinden **Desktop development with C++** iş yükünü (Windows 10/11
SDK dahil) kurun. Ardından **x64 Native Tools Command Prompt for VS** açıp şu
komutları çalıştırın:

```bat
where cl
where link
cargo test
cargo run -- examples\hello.earth
```

## Katkı rehberi

### Yeni bir insan dili eklemek

`src/keywords.rs` içindeki `KEYWORDS` ve `BUILTINS` tablolarına yeni dildeki
yazımları ekleyin. Lexer, parser ve yorumlayıcıda değişiklik gerekmez.
`spellings_are_unique` testi çakışan yazımları yakalar.

### Yeni bir yerleşik fonksiyon eklemek

1. `src/keywords.rs` içinde `Builtin` enum'una bir varyant ve `BUILTINS`
   tablosuna yazımlarını ekleyin.
2. `src/interpreter.rs` içinde `call_builtin` fonksiyonuna argüman sayısını ve
   davranışı ekleyin.
3. `src/lib.rs` içine testler ekleyin ve bu README'deki tabloyu güncelleyin.

### Yeni bir sözdizimi eklemek

Sırasıyla `keywords.rs` (gerekirse anahtar kelime), `lexer.rs` (yeni
semboller), `ast.rs`, `parser.rs` ve `interpreter.rs` dosyalarını genişletin.
Her `examples/*.earth` dosyası `tests/examples.rs` tarafından çalıştırılır.

## Yol haritası

- [x] Değişkenler ve temel değerler
- [x] İfadeler ve aritmetik
- [x] Koşullar ve döngüler
- [x] Fonksiyonlar, özyineleme ve kapanışlar
- [x] Listeler
- [x] Konum bilgili hata mesajları ve REPL
- [ ] Sözlükler (anahtar-değer yapıları)
- [ ] Hata mesajlarının da çok dilli olması
- [ ] Modüller / dosya içe aktarma
- [ ] Daha fazla insan dili
- [ ] Editör desteği (sözdizimi renklendirme)
