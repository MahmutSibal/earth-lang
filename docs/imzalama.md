# earth.exe'yi imzalamak

Windows, internetten indirilen ve **dijital imzası olmayan** programlar için
SmartScreen uyarısı ("Windows bilgisayarınızı korudu") gösterir. Uyarıda
yayıncı "Bilinmeyen yayımcı" olarak görünür. Bu uyarıyı kalıcı olarak
kaldırmanın yolu `earth.exe`'yi bir kod imzalama sertifikasıyla imzalamaktır.

İmza, dosyanın kimden geldiğini ve sonradan değiştirilmediğini kanıtlar.
Sertifika, kimliği doğrulanmış bir kişiye veya kuruluşa verilir; bu yüzden
başvuruyu proje sahibinin kendisi yapmalıdır.

## Seçenekler

| Seçenek | Ücret | Not |
|---|---|---|
| [SignPath Foundation](https://signpath.org) | Açık kaynak projelere ücretsiz | Proje için başvuru yapılır; imzalama GitHub Actions'tan yapılır. |
| [Azure Artifact Signing (eski adıyla Trusted Signing)](https://learn.microsoft.com/azure/trusted-signing/) | Aylık yaklaşık 10 $ | Microsoft'un hizmeti; SmartScreen itibarı hızlı oluşur. `release` workflow'u buna hazır. |
| Klasik OV / EV sertifika (Sectigo, DigiCert, …) | Yıllık ~200–500 $ | EV sertifika SmartScreen'de hemen güven sağlar. |

## Azure Artifact Signing ile otomatik imzalama

`.github/workflows/release.yml`, aşağıdaki depo gizli değerleri
(Settings → Secrets and variables → Actions) tanımlandığında `earth.exe`'yi
her sürümde otomatik olarak imzalar. Tanımlı değillerse imzalama adımı atlanır.

| Gizli değer | Açıklama |
|---|---|
| `AZURE_TENANT_ID` | Azure kiracı kimliği |
| `AZURE_CLIENT_ID` | İmzalama yetkisi verilen uygulama kaydının kimliği |
| `AZURE_CLIENT_SECRET` | O uygulama kaydının gizli anahtarı |
| `AZURE_SIGNING_ENDPOINT` | Örneğin `https://weu.codesigning.azure.net/` |
| `AZURE_SIGNING_ACCOUNT` | Artifact Signing hesap adı |
| `AZURE_SIGNING_PROFILE` | Sertifika profili adı |

## İmza olmadan güven

İmza alınana kadar şunlar kullanıcıların güvenmesine yardımcı olur:

- **PowerShell ile kurulum** (`install.ps1`): PowerShell'in indirdiği dosyalar
  "internetten indirildi" işareti almadığı için SmartScreen uyarısı çıkmaz.
  Betik açık kaynaktır ve indirdiği dosyayı SHA-256 ile doğrular.
- **SHA-256 özetleri:** her sürümde `SHA256SUMS.txt` yayınlanır. Elle
  indirenler `Get-FileHash earth-windows-x64.zip` ile karşılaştırabilir.
- **Kaynaktan derleme:** kod açık olduğu için herkes `cargo build --release`
  ile kendi exe'sini üretebilir.
- **İtibar:** imzasız bir dosya bile zamanla çok kişi tarafından
  indirildikçe SmartScreen itibarı kazanır ve uyarı azalır.
