<p align="center">
  <img src="docs/logo.png" alt="Earth" width="320">
</p>

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

## Earth Studio

Earth kendi editörüyle gelir: solda dosya gezgini, ortada sekmeli ve renkli
kod editörü, altta programın çıktısının göründüğü terminal.

![Earth Studio](docs/studio.png)

- **▶ Çalıştır (F5):** programı çalıştırır. Çıktı terminalde anında görünür;
  `girdi` terminale yazılanı okur.
- **⏹ Durdur (Shift+F5):** sonsuz döngüye giren programı durdurur.
- **Hata gösterimi:** hata veren satır editörde kırmızıyla işaretlenir.
- **REPL:** program çalışmıyorken terminale Earth komutları yazılabilir.
- **Dosya işlemleri:** klasör açma, yeni dosya, kaydetme (Ctrl+S).
  Kaydedilmemiş değişiklikler kapatırken sorulur.
- **Arayüz dili:** Türkçe ya da İngilizce.

Örnek programlar Studio'nun içinde gelir: ilk açılışta `Belgeler\Earth`
klasörü oluşturulur ve örnekler `Belgeler\Earth\Örnekler` altına konur.
**📚 Örnekler** düğmesi onları istediğiniz zaman geri getirir.

## Kurulum

Rust ya da başka bir şey kurmaya gerek yok.

### Windows

[Releases](https://github.com/MahmutSibal/earth-lang/releases/latest)
sayfasından **`EarthStudio-Kurulum.exe`** dosyasını indirip çalıştırın:

1. Hoş geldin ekranı
2. Kurulum klasörü (varsayılan: `%LOCALAPPDATA%\Programs\Earth Studio`;
   yönetici izni gerekmez)
3. Masaüstü kısayolu seçeneği
4. İlerleme çubuğuyla kurulum
5. "Earth Studio'yu çalıştır" ile bitiş

Kurulum klasöründe yalnızca `earth-studio.exe` ve kaldırıcı bulunur; dil,
yorumlayıcı ve örnekler programın içine gömülüdür. `.earth` dosyalarına çift
tıklamak onları Earth Studio'da açar. Kaldırmak için: **Ayarlar → Uygulamalar
→ Earth Studio**.

**Tek satırla kurulum (önerilen):** PowerShell'e şunu yapıştırın. Aynı
kurulumu indirir, SHA-256 özetini doğrular ve çalıştırır. Bu yolla
indirilen dosyalar Windows SmartScreen uyarısı göstermez.

```powershell
irm https://raw.githubusercontent.com/MahmutSibal/earth-lang/main/install.ps1 | iex
```

Kurulum dosyası henüz dijital olarak imzalı değil. Bu yüzden tarayıcıdan
indirilip açıldığında Windows "bilinmeyen yayımcı" uyarısı gösterebilir. Bu,
imzasız her yeni program için görülen bir uyarıdır ve programın zararlı olduğu
anlamına gelmez. Dosyayı aynı sayfadaki `SHA256SUMS.txt` ile
`Get-FileHash EarthStudio-Kurulum.exe` komutuyla doğrulayabilirsiniz. İmzalama
planı için [docs/imzalama.md](docs/imzalama.md) dosyasına bakın.

### Linux ve macOS

Releases sayfasındaki `earth-studio-linux-x64.tar.gz` ya da
`earth-studio-macos-arm64.tar.gz` arşivini açıp `earth-studio` programını
çalıştırın.

## Kaynaktan derleme

[Rust](https://www.rust-lang.org/tools/install) kurun, sonra:

```bash
cargo run -p earth-studio                      # Earth Studio'yu aç
cargo run -p earth-studio -- examples/fizzbuzz.earth
```

Windows kurulum dosyasını Linux'ta üretmek (MinGW-w64, Wine ve Wine altında
kurulu [Inno Setup](https://jrsoftware.org/isinfo.php) gerekir):

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release -p earth-studio --target x86_64-pc-windows-gnu
cd windows && wine 'C:\InnoSetup\ISCC.exe' /DAppVersion=0.5.0 \
  '/DStudioExe=..\target\x86_64-pc-windows-gnu\release\earth-studio.exe' installer.iss
```

Sonuç `windows/Output/EarthStudio-Kurulum.exe` olur. Yeni bir sürüm
yayınlamak için bir etiket gönderin (`git tag v0.5.0 && git push origin
v0.5.0`); `release` workflow'u kurulum dosyasını, Linux/macOS arşivlerini ve
`SHA256SUMS.txt`'yi Releases sayfasına ekler.

Kontroller:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
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
| `dene` / `yakala` | | `try` / `catch` | hata yakalama |
| `fırlat` | `firlat` | `throw` | hata fırlatma |
| `içe_aktar` | `ice_aktar` | `import` | başka bir dosyayı yükleme |

### Değerler ve işlemler

- **Sayılar:** `42`, `3.14`. İşlemler: `+ - * / %`. Sıfıra bölme hata verir.
- **Metinler:** `"Merhaba"`. Kaçış dizileri: `\" \\ \n \r \t`. `+` ile bir metne
  herhangi bir değer eklenebilir: `"x = " + 5`.
- **Listeler:** `[1, "iki", doğru]`. İndeksleme `liste[0]`, sondan saymak için
  `liste[-1]`, atama `liste[0] = 5`. `+` iki listeyi birleştirir.
- **Sözlükler:** `{"ad": "Ada", "yaş": 36}`. Anahtarlar metindir. Okuma
  `kişi["ad"]`, ekleme ve güncelleme `kişi["şehir"] = "Londra"`. Olmayan bir
  anahtarı okumak hata verir, önce `içerir(kişi, "şehir")` ile kontrol
  edilebilir. Sözlükler ekleme sırasını korur. `her k içinde kişi` anahtarlar
  üzerinde döner.
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
| `sil(liste, indeks)`, `sil(sözlük, anahtar)` | `remove` | elemanı çıkarır ve döndürür |
| `içerir(x, aranan)` | `contains` | listede eleman, sözlükte anahtar, metinde alt metin var mı |
| `anahtarlar(sözlük)` | `keys` | anahtarların listesi |
| `değerler(sözlük)` | `values` | değerlerin listesi |
| `metin(x)` | `str` | değeri metne çevirir |
| `sayı(x)` | `number` | metni sayıya çevirir; olmazsa `boş` döner |
| `tür(x)` | `type` | `"sayı"`, `"metin"`, `"liste"`, `"sözlük"`, … |

**Metin**

| Türkçe | English | Açıklama |
|---|---|---|
| `büyük_harf(m)`, `küçük_harf(m)` | `upper`, `lower` | harf büyüklüğü; Türkçe programlarda `i`↔`İ`, `ı`↔`I` |
| `kırp(m)` | `trim` | baştaki ve sondaki boşlukları siler |
| `böl(m, ayırıcı?)` | `split` | metni parçalara ayırır; ayırıcı yoksa boşluklardan |
| `birleştir(liste, ayırıcı?)` | `join` | elemanları metin olarak birleştirir |
| `değiştir(m, eski, yeni)` | `replace` | geçen her `eski`yi `yeni` yapar |

**Matematik**

| Türkçe | English | Açıklama |
|---|---|---|
| `mutlak(x)`, `karekök(x)`, `üs(x, y)` | `abs`, `sqrt`, `pow` | |
| `yuvarla(x, basamak?)`, `taban(x)`, `tavan(x)` | `round`, `floor`, `ceil` | |
| `rastgele()`, `rastgele(n)`, `rastgele(a, b)` | `random` | [0, 1) kesir, [0, n) ya da [a, b] tam sayı |
| `en_küçük(…)`, `en_büyük(…)` | `min`, `max` | sayılar ya da bir sayı listesi |
| `topla(liste)` | `sum` | sayıların toplamı |

**Liste**

| Türkçe | English | Açıklama |
|---|---|---|
| `sırala(liste)` | `sort` | sıralanmış yeni liste (hepsi sayı ya da hepsi metin) |
| `ters(x)` | `reverse` | ters çevrilmiş yeni liste veya metin |
| `dilim(x, baş, son?)` | `slice` | `baş`tan `son`a kadarki parça; negatif indeks sondan sayar |

### Hata yakalama

```earth
fonksiyon böl_güvenli(a, b) {
    eğer b == 0 { fırlat "sıfıra bölünemez" }
    döndür a / b
}

dene {
    yaz(böl_güvenli(1, 0))
} yakala hata {
    yaz("Hata:", hata)
}
```

`fırlat` herhangi bir değeri (metin, sözlük, …) fırlatabilir; `yakala`
o değeri olduğu gibi alır. Earth'ün kendi hataları (sıfıra bölme, olmayan
indeks, …) da yakalanabilir; bu durumda hata değişkeni mesajı metin olarak
taşır. `yakala`dan sonraki değişken adı isteğe bağlıdır.

### Modüller

```earth
# geometri.earth
değişken PI = 3.14159
fonksiyon alan(r) { döndür PI * r * r }

# ana.earth
içe_aktar "geometri.earth"
yaz(alan(2))
```

`içe_aktar`, dosyayı içe aktaran dosyanın klasörüne göre bulur ve yalnızca
bir kez çalıştırır. Modül kendi kapsamında çalışır, yani içe aktaranın
değişkenlerini göremez; tanımladığı değişkenler ve fonksiyonlar içe aktarana
kopyalanır. Modüldeki bir hata, modül dosyasının adı ve satırıyla gösterilir.

### Sözdizimi notları

- Her satıra bir komut yazılır. Aynı satırda birden fazla komut için `;`
  kullanın.
- `#` ile başlayan kısım satır sonuna kadar yorumdur.
- Parantez, köşeli parantez ve sözlük içindeki ifadeler birden fazla satıra
  yayılabilir.
- `değilse`, kapanan `}` ile aynı satırda ya da bir sonraki satırda olabilir.

### Çıktı dili

Earth, programın hangi dilde yazıldığını anahtar kelimelerinden anlar ve o
dilde cevap verir. İngilizce yazılmış bir program `true`, `nil` ve İngilizce
hata mesajları görür; Türkçe yazılmış bir program `doğru`, `boş` ve Türkçe
hata mesajları görür. Earth Studio'nun durum çubuğu, açık dosyanın hangi
dilde algılandığını gösterir.

### Hata mesajları

Hatalar satır ve sütun bilgisiyle Earth Studio'nun terminalinde gösterilir;
hatalı satır editörde kırmızıyla işaretlenir:

```text
✖ Hata: program.earth:2:14: '*' işlemi 'metin' ve 'sayı' türleri arasında yapılamaz
```

## Editör desteği

`editors/vscode` klasörü VS Code için sözdizimi renklendirme eklentisidir.
Kurulum için [editors/vscode/README.md](editors/vscode/README.md) dosyasına
bakın.

## Mimari

- **`src/keywords.rs`:** dilin çok dilli sözlüğü. Anahtar kelimelerin ve
  yerleşik fonksiyonların bütün yazımları buradadır.
- **`src/messages.rs`:** kullanıcıya gösterilen bütün mesajlar, her dilde.
- **`src/lexer.rs`:** kaynak metni satır/sütun bilgili token'lara ayırır.
- **`src/parser.rs`, `src/ast.rs`:** öncelik sıralı özyinelemeli iniş
  (recursive descent) ayrıştırıcı ve sözdizimi ağacı. `kır` / `döndür` gibi
  yanlış yerde kullanılan komutları çalıştırmadan önce yakalar.
- **`src/interpreter.rs`, `src/value.rs`:** ağaç gezen yorumlayıcı, değerler,
  kapsamlar, hata yakalama ve modüller.
- **`src/stdlib.rs`:** girdi/çıktı dışındaki yerleşik fonksiyonlar.
- **`studio/`:** Earth Studio editörü (egui). Programları ayrı bir iş
  parçacığında çalıştırır ve yorumlayıcının `set_interrupt` desteğiyle
  durdurur. Dil çekirdeği (`src/`) bağımlılıksız kalır; grafik
  kütüphaneleri yalnızca Studio'da kullanılır.

Dil çekirdeği yalnızca Rust standart kütüphanesini kullanır.

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
cargo run -p earth-studio
```

## Katkı rehberi

### Yeni bir insan dili eklemek

1. `src/keywords.rs` içinde `Lang` enum'una yeni dili ekleyin ve `KEYWORDS` ile
   `BUILTINS` tablolarına o dilin yazımları için yeni bir sütun açın.
2. `src/messages.rs` içinde tür adlarını, mesajları ve CLI metinlerini o dile
   çevirin. Derleyici eksik çevirileri gösterir.

Lexer, parser ve yorumlayıcıda değişiklik gerekmez.
`spellings_are_unique_and_complete` testi çakışan ya da eksik yazımları yakalar.

### Yeni bir yerleşik fonksiyon eklemek

1. `src/keywords.rs` içinde `Builtin` enum'una bir varyant ve `BUILTINS`
   tablosuna yazımlarını ekleyin.
2. `src/stdlib.rs` içinde `arity` ve `call` fonksiyonlarına argüman sayısını
   ve davranışı ekleyin.
3. Editör renklendirmesini yeniden üretin:
   `python3 editors/vscode/gen_grammar.py src/keywords.rs editors/vscode/syntaxes/earth.tmLanguage.json`
4. `src/lib.rs` içine testler ekleyin ve bu README'deki tabloyu güncelleyin.

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
- [x] Sözlükler (anahtar-değer yapıları)
- [x] Hata mesajlarının ve çıktının çok dilli olması
- [x] Standart kütüphane (metin, matematik, liste)
- [x] Hata yakalama (`dene` / `yakala` / `fırlat`)
- [x] Modüller / dosya içe aktarma
- [ ] Daha fazla insan dili
- [x] Editör desteği (VS Code sözdizimi renklendirme)
- [x] Windows kurulum programı (hoş geldin ekranı, masaüstü kısayolu, kaldırıcı)
- [x] Earth Studio: gezgin, sekmeli editör ve terminal içeren editör
- [ ] Sınıflar / kullanıcı tanımlı türler
- [ ] Dosya okuma ve yazma
- [ ] Daha hızlı çalışma için bytecode derleyici
