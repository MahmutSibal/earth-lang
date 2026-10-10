# Earth için VS Code eklentisi

`.earth` dosyaları için sözdizimi renklendirme: Türkçe ve İngilizce anahtar
kelimeler, yerleşik fonksiyonlar, metinler, sayılar ve yorumlar. Ayrıca `#`
ile yorum satırı ekleme ve parantezlerin otomatik kapanması da destekleniyor.

## Kurulum

Bu klasörü VS Code eklenti klasörüne kopyalayın ve VS Code'u yeniden başlatın:

- Windows: `%USERPROFILE%\.vscode\extensions\earth-lang`
- Linux / macOS: `~/.vscode/extensions/earth-lang`

## Anahtar kelimeler değişince

`syntaxes/earth.tmLanguage.json` dosyası `src/keywords.rs` içindeki
tablolardan üretilir. Depo kök klasöründe şunu çalıştırın:

```bash
python3 editors/vscode/gen_grammar.py src/keywords.rs editors/vscode/syntaxes/earth.tmLanguage.json
```
