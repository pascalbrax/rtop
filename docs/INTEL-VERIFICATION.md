# Intel Arc — backend e verifica

Implementato il 4 ottobre 2026. Il 6 ottobre l'utente ha fornito una diagnostica reale da HP ZBook con Intel Ultra 7 e Intel Arc: discovery confermata con driver `i915`, ma metriche GPU ancora parzialmente indisponibili. Non è presente una GPU Intel nell'ambiente di sviluppo.

## Supporto e capacità

Discovery DRM per vendor `0x8086` con driver `i915` o `xe`, identificazione per indirizzo PCI e lettura dei sensori hwmon. La libreria `libze_loader.so.1` viene caricata a runtime soltanto quando viene rilevata una GPU Intel con uno di questi driver; non è necessaria per compilare rtop o per avviarlo senza Intel.

Il backend usa Level Zero Sysman, con inizializzazione `zesInit`, enumerazione driver/device e associazione PCI al dispositivo DRM. I loader precedenti a `zesInit` producono una diagnostica; non viene modificato l'ambiente del processo multithread con `ZES_ENABLE_SYSMAN`. Il modulo resta caricato fino alla terminazione del processo (`RTLD_NODELETE`), rispettando l'inizializzazione Sysman globale e senza forzare l'unload di un runtime attivo.

| Metrica | Definizione e limiti |
| --- | --- |
| Utilizzo GPU | Differenze di `activeTime` e `timestamp` del gruppo engine `ALL` dell'intero dispositivo, ×100. Non vengono sommati engine sovrapposti o scelti singoli engine come sostituto del totale. |
| VRAM | Moduli con location `DEVICE`; memoria di sistema esclusa. Totale fisico quando disponibile, altrimenti `state.size` per runtime precedenti; usata = totale − libera. Moduli root e tile non vengono sommati insieme. |
| Temperature | Domini Sysman con etichetta del tipo e tile, quando presente; hwmon resta indipendente e continua anche senza runtime Sysman. Il massimo rappresentabile dal sensore non viene spacciato per limite critico. |
| Potenza | Non raccolta dal backend Intel; indicata come `N/D`. |

Le capacità dipendono da GPU, kernel, runtime e permessi: il riconoscimento DRM non garantisce tutte le metriche. In particolare, se manca il gruppo engine `ALL`, l'utilizzo totale rimane `N/D`. Una GPU Intel integrata senza memoria locale non mostra RAM di sistema come VRAM.

Il primo campione dei contatori è `collecting`; reset, assenza di progresso ed errori interrompono la curva. La pausa azzera le basi alla ripresa. Valori di utilizzo fuori 0–100%, contatori regressivi e totali VRAM invalidi vengono rifiutati, senza clamp che possa nascondere un errore del driver.

Il worker hardware, la frequenza predefinita di due secondi, discovery ogni 30 secondi e slot singolo restano quelli di M4. Nessun subprocess periodico, scansione dei processi per stimare utilizzo GPU o scrittura ai controlli del dispositivo. Inizializzazione lenta ed errori del runtime non bloccano tastiera e collector di base.

`rtop doctor` mostra il driver DRM Intel, indirizzo PCI, backend, metriche e motivi delle assenze. La chiusura anticipata della pipe di output termina normalmente, senza panic.

## Test utente HP ZBook — 6 ottobre 2026

Fonti: diagnostiche `doctor` con e senza `sudo` e informazioni di sistema fornite dall'utente.

| Ambiente | Dato dichiarato |
| --- | --- |
| Portatile | HP ZBook Firefly 16 inch G11 Mobile Workstation PC |
| Distribuzione | Fedora Linux 44, KDE Plasma Desktop Edition |
| Kernel | `7.1.13-200.fc44.x86_64`, x86_64, PREEMPT_DYNAMIC; build 2 settembre 2026 |
| CPU | Intel Core Ultra 7 155H, 22 CPU logiche riportate dal sistema |
| GPU | Meteor Lake-P [Intel Arc Graphics], PCI `0000:00:02.0`, ID `8086:7d55`, driver `i915` dalla diagnostica |
| BIOS | HP W70 Ver. 01.06.02, 9 maggio 2025 |

Versioni dei pacchetti installati, fornite dall'utente tramite `rpm -q`:

| Pacchetto | Versione |
| --- | --- |
| `oneapi-level-zero` (loader) | `1.28.6-1.fc44.x86_64` |
| `intel-level-zero` | `26.22.38646.6-4.fc44.x86_64` |
| `intel-compute-runtime` | `26.22.38646.6-4.fc44.x86_64` |

Il runtime è installato e la diagnostica raggiunge l'enumerazione Sysman; la mancanza del contatore aggregato non viene attribuita a un pacchetto assente. Le versioni RPM identificano i pacchetti installati, senza verificare da sole il percorso delle librerie effettivamente caricate. La revisione del binario rtop resta non specificata.

| Voce | Utente normale | Con `sudo` |
| --- | --- | --- |
| Discovery Intel | `card1`, **i915**, PCI `0000:00:02.0`, ID `8086:7d55`, Level Zero Sysman | Invariata |
| Utilizzo GPU | `Intel Sysman insufficient permissions (0x70010000)` | `Intel whole-device engine counter unavailable` |
| VRAM locale | `Intel device-local VRAM unavailable` | Invariata |
| Temperatura GPU | `Intel temperature sensors unavailable` | Invariata |
| Potenza GPU | Non raccolta dal backend Intel | Invariata |
| Temperatura CPU | Package 37 °C, core 29–37 °C | Package 35 °C, core 28–35 °C |
| Limite critico CPU dichiarato dal sensore | 110 °C | 110 °C |
| Altri sensori | ACPI, NVMe, DIMM `spd5118`, Wi-Fi come `Other` | Ancora enumerati come `Other` |
| Costo singolo comando | GPU/discovery 44,009 ms; sensori 30,450 ms | GPU/discovery 44,954 ms; sensori 30,066 ms |

Il comando termina con un report anche quando le metriche GPU non sono disponibili. Questo conferma discovery e lettura dei sensori CPU su hardware reale; non verifica stabilità della TUI, correttezza delle metriche GPU sotto carico o consumo continuativo. I costi di `doctor` comprendono discovery e non sono un benchmark del worker a regime.

Compare anche `Intel device PCI: Intel Sysman uninitialized (0x78000001)`: la diagnosi PCI del runtime non è completamente riuscita, pur essendo presente il dispositivo DRM. L'assenza di NVML non riguarda il backend Intel. L'errore di permessi riguarda l'utilizzo GPU: non dimostra che VRAM e temperatura siano recuperabili con gli stessi permessi. L'assenza di memoria locale è compatibile con una GPU integrata; rtop non sostituisce la VRAM con RAM di sistema.

L'utente ha fornito anche l'esecuzione di `sudo ./target/release/rtop doctor`. L'errore di permessi scompare, ma non viene trovato un contatore engine root di tipo `ALL`, richiesto dal collector attuale per rappresentare l'utilizzo dell'intera GPU. Questo messaggio viene prodotto dopo l'enumerazione degli engine senza trovare quel gruppo; il report non elenca gli engine disponibili e non permette di concludere che ogni contatore GPU sia assente. Non è un caso di primo campione `collecting`.

Il confronto dimostra che i privilegi da soli non rendono disponibili le metriche richieste. Temperatura GPU, memoria locale e diagnostica PCI `uninitialized` restano invariate. Non si attribuisce la causa esatta a hardware, kernel o versione del runtime senza ulteriori dati. Distribuzione, kernel e modello sono ora identificati. Le versioni loader/runtime sono ora registrate; per valutare un'estensione del backend resta da ottenere un elenco dei gruppi engine effettivamente esposti. Non sono state applicate modifiche ai permessi o ai parametri del kernel.

## Prova su Fedora

```bash
git pull
cargo build --release --locked
./target/release/rtop doctor
./target/release/rtop
```

Per identificare GPU e driver effettivi:

```bash
lspci -nnk | grep -A3 -E 'VGA|3D|Display'
```

Se `doctor` segnala il loader o il runtime Intel mancanti, Fedora fornisce [oneapi-level-zero](https://packages.fedoraproject.org/pkgs/oneapi-level-zero/oneapi-level-zero/) (loader) e [intel-level-zero](https://packages.fedoraproject.org/pkgs/intel-compute-runtime/intel-level-zero/) (runtime GPU):

```bash
sudo dnf install oneapi-level-zero intel-level-zero
```

Questa istruzione usa i pacchetti della distribuzione: non richiede cambi di driver kernel. La disponibilità delle metriche va verificata con il modello e la versione Fedora effettivi. Per diagnosticare soltanto DRM e hwmon:

```bash
./target/release/rtop doctor --disable-intel
```

`doctor` raccoglie un campione: l'utilizzo Intel può ancora essere `collecting`. Nella TUI attendere il secondo campione; `--live-preview` ora inizializza le basi anche del collector hardware prima della seconda lettura.

## Test eseguiti

17 test Rust, Clippy con warning come errori, formattazione, smoke test terminale e validazione CLI/TOML superati. Le regressioni NVML di M4 passano anche con il backend Intel presente.

`tests/intel_safety.py` compila una fixture C temporanea e verifica i due driver DRM `xe`/`i915`, associazione PCI senza duplicare le GPU, utilizzo su due campioni, VRAM locale, esclusione della RAM di sistema, assenza di doppio conteggio root/tile, temperature e pausa/ripresa. Copre inoltre zero driver/device, init fallita, errori di conteggio, handle nulli, conteggio eccessivo o modificato, PCI diverso/non leggibile, permessi negati, feature non supportate, GPU persa, valori invalidi, simboli opzionali assenti e loader precedente a `zesInit`.

Con init fittizia bloccata per 10 secondi, q termina l'app entro due secondi e ripristina il terminale. I test verificano anche `doctor` con pipe già chiusa. Report: [intel-safety.json](benchmarks/intel-safety.json), [regressione NVML](benchmarks/intel-nvml-regression.json).

L'ABI Rust è verificata indipendentemente contro gli header ufficiali Level Zero v1.19-r1.19.12 tramite `tests/fixtures/intel_abi_layouts.c`: dimensioni, offset ed enum su Linux a 64 bit; `ze_bool_t` è un byte. Esito: [intel-abi-verification.txt](benchmarks/intel-abi-verification.txt).

```bash
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline
python3 tests/intel_safety.py
python3 tests/hardware_safety.py
```

Gli header ufficiali servono soltanto alla verifica ABI opzionale, non al test con fixture o alla build Rust:

```bash
cc -std=c11 -I/path/to/level-zero/include tests/fixtures/intel_abi_layouts.c -o /tmp/rtop-intel-abi
/tmp/rtop-intel-abi
```

## Prestazioni e validazione hardware

Controllo della dashboard release per 300,000 secondi misurati dopo 30 secondi iniziali, su PTY drenato 120×40 truecolor, con CPU/RAM/dischi/rete e dieci sensori CPU reali. Non è presente Intel nell'ambiente locale: questo controllo misura il percorso senza Intel e non il costo di Sysman su Arc reale.

| Misura | Risultato |
| --- | --- |
| CPU media di un core | 0,423% (budget ≤0,5%) |
| RSS iniziale / finale | 4096 / 4224 KiB; aumento di 128 KiB, stabile a 4224 KiB dal campione a 90 s |
| Thread massimi | 4 |
| Latenza input-render, 40 campioni | p50 3,68 ms; p95 4,05 ms; massimo 4,61 ms |
| Uscita con init Intel fittizia bloccata | 3,37 ms; terminale ripristinato |
| Ripristino dopo il test di stabilità | Verificato, uscita 0 |

Latenza misurata su un processo separato, dopo 125 secondi (storici base pieni, circa 63 campioni termici). Il test è stato eseguito durante il controllo di stabilità; il costo CPU è attribuito al singolo processo. Il costo dell'emulatore grafico e della latenza SSH resta escluso. Questa singola finestra breve è un controllo di regressione, non sostituisce il protocollo esteso M4 o un benchmark Intel reale.

Report: [dashboard](benchmarks/intel-absent-tui.json), [campioni CSV](benchmarks/intel-absent-tui.csv), [latenza](benchmarks/intel-input-latency.json), [diagnostica locale](benchmarks/intel-local-doctor.txt). Hash release: `291c9660512ade43e7e29f1395492234c62230d607be87281f42cea71817caf9`.

Riproduzione:

```bash
cargo build --release --locked
python3 tests/tui_soak.py --seconds 300 --output docs/benchmarks/intel-absent-tui
python3 tests/tui_latency.py --output docs/benchmarks/intel-input-latency.json
```

Dopo il report ZBook, resta da registrare la revisione del binario; verificare le capacità engine effettive oltre al limite di permessi e le capacità di temperatura/memoria GPU; confrontare le metriche e misurare latenza e consumo sulla macchina Intel. La verifica AMD reale è in sospeso su richiesta dell'utente; il codice AMD e le sue fixture restano attivi.

## Riferimenti

- [API Level Zero Sysman](https://oneapi-src.github.io/level-zero-spec/level-zero/1.16.24/sysman/api.html)
- [Header ufficiale zes_api.h](https://github.com/oneapi-src/level-zero/blob/master/include/zes_api.h)
- [Runtime Intel e hardware supportato](https://dgpu-docs.intel.com/overview/hardware-table.html)
