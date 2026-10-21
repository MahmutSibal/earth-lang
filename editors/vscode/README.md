# Earth Language için VS Code eklentisi

`.earth` dosyaları için sözdizimi renklendirme: Türkçe ve İngilizce anahtar
kelimeler, yerleşik fonksiyonlar, metinler, sayılar ve yorumlar. Ayrıca `#`
ile yorum satırı ekleme ve parantezlerin otomatik kapanması da destekleniyor.

## Earth nedir?

Earth, programları Türkçe veya İngilizce yazabileceğiniz, başlangıç seviyesine
uygun açık kaynaklı bir programlama dilidir. Earth Studio editörü, öğretmen
modu ve görsel hata ayıklayıcısı ile birlikte gelir.

```earth
değişken ad = "Dünya"
yaz("Merhaba, " + ad + "!")
```

## Özellikler

- Türkçe ve İngilizce anahtar kelimeler ve yerleşik fonksiyonlar
- `.earth` dosyaları için dil tanıma
- Sözdizimi renklendirme
- Yorumlar, metinler, sayılar ve operatörler için renklendirme
- Parantezlerin otomatik kapanması
- F5 ile açık Earth dosyasını çalıştırma
- Sağ tık menüsünden Earth Studio'da açma
- Earth komutunun çıktısını VS Code Output panelinde görme

## Earth Studio entegrasyonu

Earth Studio kurulduğunda ve `earth` komutu PATH'e eklendiğinde ek ayar
gerekmez. Bir `.earth` dosyası açıkken:

- **F5** veya sağ tık → **Earth: Dosyayı çalıştır** dosyayı kaydeder ve
  `earth` komutuyla çalıştırır.
- Sağ tık → **Earth: Earth Studio'da aç** dosyayı Earth Studio'da açar.
- Komut Paleti → **Earth: Ayarları aç** eklenti ayarlarını gösterir.

Earth komutu PATH'te değilse ayarlardan tam yolu belirtin:

```json
{
  "earth.executablePath": "C:\\Users\\Siz\\AppData\\Local\\Programs\\Earth Studio\\earth.exe",
  "earth.studioPath": "C:\\Users\\Siz\\AppData\\Local\\Programs\\Earth Studio\\earth-studio.exe"
}
```

Linux veya macOS'ta karşılık gelen çalıştırılabilir dosya yollarını kullanın.
Eklenti Windows'ta PATH'i ve varsayılan Earth Studio kurulum klasörünü de
otomatik olarak kontrol eder.

## Kurulum

VS Code içinde **Extensions** panelini açın ve `Earth` aratın. Eklentiyi
`mahmutsibal.earth-lang` yayıncısından seçip **Install** düğmesine basın.

Earth Studio ve dilin kaynak kodu için
[Earth GitHub deposunu](https://github.com/MahmutSibal/earth-lang) ziyaret edin.

## Geliştiriciler için yerel kurulum

Bu klasörü VS Code eklenti klasörüne kopyalayın ve VS Code'u yeniden başlatın:

- Windows: `%USERPROFILE%\.vscode\extensions\earth-lang`
- Linux / macOS: `~/.vscode/extensions/earth-lang`

## Anahtar kelimeler değişince

`syntaxes/earth.tmLanguage.json` dosyası `src/keywords.rs` içindeki
tablolardan üretilir. Depo kök klasöründe şunu çalıştırın:

```bash
python3 editors/vscode/gen_grammar.py src/keywords.rs editors/vscode/syntaxes/earth.tmLanguage.json
```
