# Intel Arc — backend e verifica

Implementato il 4 ottobre 2026. Test utente previsto su Fedora; modello GPU ancora da identificare, driver kernel probabilmente `xe` (non ancora verificato). Non è stata eseguita una prova su Intel Arc reale nell'ambiente di sviluppo.

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

Restano da registrare modello GPU, versione Fedora/kernel/runtime, metriche disponibili, confronto con strumenti appropriati, latenza e consumo sulla macchina Fedora. La verifica AMD reale è in sospeso su richiesta dell'utente; il codice AMD e le sue fixture restano attivi.

## Riferimenti

- [API Level Zero Sysman](https://oneapi-src.github.io/level-zero-spec/level-zero/1.16.24/sysman/api.html)
- [Header ufficiale zes_api.h](https://github.com/oneapi-src/level-zero/blob/master/include/zes_api.h)
- [Runtime Intel e hardware supportato](https://dgpu-docs.intel.com/overview/hardware-table.html)
