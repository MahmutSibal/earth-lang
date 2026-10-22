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

## Bağımsız Earth CLI ile çalıştırma

Eklenti yalnızca sözdizimi renklendirme ve bağımsız Earth CLI çalıştırma
sağlar; F5 için Earth Studio kurulması gerekmez. Bir `.earth` dosyası açıkken:

- **F5** veya sağ tık → **Earth: Dosyayı çalıştır** dosyayı kaydeder ve
  yalnızca `earth`/`earth.exe` CLI ile çalıştırır.
- Komut Paleti → **Earth: Ayarları aç** eklenti ayarlarını gösterir.

CLI PATH'te değilse ayarlardan tam yolu belirtin:

```json
{
  "earth.cliPath": "C:\\Users\\Siz\\AppData\\Local\\Programs\\Earth\\earth.exe",
  "earth.studioPath": "C:\\Users\\Siz\\AppData\\Local\\Programs\\Earth Studio\\earth-studio.exe"
}
```

Linux veya macOS'ta karşılık gelen CLI yolunu kullanın. Eklenti PATH'in yanı
sıra platforma uygun varsayılan CLI konumlarını da kontrol eder. CLI
bulunamazsa eklenti açık bir hata gösterir ve CLI release sayfasını açmayı
önerir; Earth Studio'yu indirmez veya zorunlu kılmaz.

## İsteğe bağlı Earth Studio entegrasyonu

Sağ tık → **Earth: Earth Studio'da aç** komutu yalnızca açıkça seçildiğinde
kullanılır. Studio kurulu değilse eklenti ayarları açmayı veya release
sayfasına gitmeyi önerir; F5 akışını etkilemez.

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
