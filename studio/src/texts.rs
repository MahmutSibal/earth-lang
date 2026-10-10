//! Every piece of text Earth Studio shows, in each interface language.

use earth::Lang;

pub struct Texts {
    pub open_folder: &'static str,
    pub examples: &'static str,
    pub new_file: &'static str,
    pub save: &'static str,
    pub run: &'static str,
    pub stop: &'static str,
    pub explorer: &'static str,
    pub no_folder: &'static str,
    pub refresh: &'static str,
    pub new_file_in_folder: &'static str,
    pub file_name_hint: &'static str,
    pub create: &'static str,
    pub cancel: &'static str,
    pub terminal: &'static str,
    pub clear: &'static str,
    pub ready: &'static str,
    pub running: &'static str,
    pub waiting_for_input: &'static str,
    pub input_hint: &'static str,
    pub repl_hint: &'static str,
    pub line_column: &'static str,
    pub untitled: &'static str,
    pub finished: &'static str,
    pub stopped: &'static str,
    pub saved: &'static str,
    pub unsaved_title: &'static str,
    pub unsaved_body: &'static str,
    pub save_all_and_quit: &'static str,
    pub quit_without_saving: &'static str,
    pub close_tab_unsaved: &'static str,
    pub close_without_saving: &'static str,
    pub error_in: &'static str,
    pub could_not_open: &'static str,
    pub could_not_save: &'static str,
    pub already_exists: &'static str,
    pub earth_files: &'static str,
    pub welcome: &'static str,
    pub downloads: &'static str,
    pub downloads_hint: &'static str,
    pub up_to_date: &'static str,
    pub update_downloading: &'static str,
    pub update_ready: &'static str,
    pub update_now: &'static str,
    pub update_later: &'static str,
    pub update_available: &'static str,
    pub update_download: &'static str,
    pub check_updates: &'static str,
}

const TURKISH: Texts = Texts {
    open_folder: "📂 Klasör Aç",
    examples: "📚 Örnekler",
    new_file: "📄 Yeni",
    save: "💾 Kaydet",
    run: "▶ Çalıştır",
    stop: "⏹ Durdur",
    explorer: "GEZGİN",
    no_folder: "Henüz bir klasör açılmadı.",
    refresh: "⟳",
    new_file_in_folder: "+ Dosya",
    file_name_hint: "dosya_adı.earth",
    create: "Oluştur",
    cancel: "İptal",
    terminal: "TERMİNAL",
    clear: "Temizle",
    ready: "Hazır",
    running: "Çalışıyor…",
    waiting_for_input: "Girdi bekleniyor",
    input_hint: "Programa girdi yazıp Enter'a basın",
    repl_hint: "Earth komutu yazıp Enter'a basın, örn. yaz(2 + 3)",
    line_column: "Satır {line}, Sütun {column}",
    untitled: "adsız",
    finished: "Program bitti",
    stopped: "Program durduruldu",
    saved: "Kaydedildi",
    unsaved_title: "Kaydedilmemiş değişiklikler",
    unsaved_body: "Bazı dosyalarda kaydedilmemiş değişiklikler var.",
    save_all_and_quit: "Kaydet ve çık",
    quit_without_saving: "Kaydetmeden çık",
    close_tab_unsaved: "Bu dosyadaki değişiklikler kaydedilmedi.",
    close_without_saving: "Kaydetmeden kapat",
    error_in: "Hata",
    could_not_open: "Açılamadı",
    could_not_save: "Kaydedilemedi",
    already_exists: "Bu adda bir dosya zaten var",
    earth_files: "Earth programları",
    welcome: WELCOME_TR,
    downloads: "⬇ {count} kişi indirdi",
    downloads_hint: "Earth Studio'nun GitHub'dan toplam indirilme sayısı",
    up_to_date: "Güncel",
    update_downloading: "Güncelleme indiriliyor: {version} (%{percent})",
    update_ready: "⬆ Earth Studio {version} hazır.",
    update_now: "Yeniden başlat ve güncelle",
    update_later: "Sonra",
    update_available: "⬆ Earth Studio {version} yayınlandı.",
    update_download: "İndir",
    check_updates: "Güncellemeleri denetlemek için tıklayın",
};

const ENGLISH: Texts = Texts {
    open_folder: "📂 Open Folder",
    examples: "📚 Examples",
    new_file: "📄 New",
    save: "💾 Save",
    run: "▶ Run",
    stop: "⏹ Stop",
    explorer: "EXPLORER",
    no_folder: "No folder is open yet.",
    refresh: "⟳",
    new_file_in_folder: "+ File",
    file_name_hint: "file_name.earth",
    create: "Create",
    cancel: "Cancel",
    terminal: "TERMINAL",
    clear: "Clear",
    ready: "Ready",
    running: "Running…",
    waiting_for_input: "Waiting for input",
    input_hint: "Type input for the program and press Enter",
    repl_hint: "Type an Earth command and press Enter, e.g. print(2 + 3)",
    line_column: "Ln {line}, Col {column}",
    untitled: "untitled",
    finished: "Program finished",
    stopped: "Program stopped",
    saved: "Saved",
    unsaved_title: "Unsaved changes",
    unsaved_body: "Some files have unsaved changes.",
    save_all_and_quit: "Save and quit",
    quit_without_saving: "Quit without saving",
    close_tab_unsaved: "This file has unsaved changes.",
    close_without_saving: "Close without saving",
    error_in: "Error",
    could_not_open: "Could not open",
    could_not_save: "Could not save",
    already_exists: "A file with this name already exists",
    earth_files: "Earth programs",
    welcome: WELCOME_EN,
    downloads: "⬇ {count} downloads",
    downloads_hint: "Total downloads of Earth Studio from GitHub",
    up_to_date: "Up to date",
    update_downloading: "Downloading update: {version} ({percent}%)",
    update_ready: "⬆ Earth Studio {version} is ready.",
    update_now: "Restart and update",
    update_later: "Later",
    update_available: "⬆ Earth Studio {version} is out.",
    update_download: "Download",
    check_updates: "Click to check for updates",
};

pub fn texts(lang: Lang) -> &'static Texts {
    match lang {
        Lang::Tr => &TURKISH,
        Lang::En => &ENGLISH,
    }
}

const WELCOME_TR: &str = r#"# Earth Studio'ya hoş geldin!
# Bu programı çalıştırmak için ▶ Çalıştır'a ya da F5'e bas.

yaz("Merhaba, Dünya!")

değişken ad = girdi("Adın ne? ")
eğer ad == "" { ad = "Dünya" }
yaz("Selam, " + ad + "!")

her i içinde aralık(1, 6) {
    yaz(i, "x", i, "=", i * i)
}
"#;

const WELCOME_EN: &str = r#"# Welcome to Earth Studio!
# Press ▶ Run or F5 to run this program.

print("Hello, World!")

let name = input("What is your name? ")
if name == "" { name = "World" }
print("Hi, " + name + "!")

for i in range(1, 6) {
    print(i, "x", i, "=", i * i)
}
"#;
