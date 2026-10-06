# M4 — GPU e temperature

Implementazione e verifiche locali del 4 ottobre 2026. La validazione completa su GPU reali resta aperta; il test AMD è in sospeso su richiesta dell’utente. Nella macchina locale è presente una Matrox, senza backend supportato; un test utente su Debian ha confermato discovery e lettura delle metriche di una NVIDIA T1000 8GB.

## Comportamento

La dashboard normale mostra GPU e sensori reali, con `N/D` per capacità mancanti. Solo i processi mantengono l'etichetta `DEMO`; `--demo` conserva tutte le fixture del prototipo. I colori e il layout approvati restano gli stessi.

NVIDIA usa `libnvidia-ml.so.1` caricata a runtime tramite `dlopen`, senza dipendenze NVIDIA in fase di link. Inizializzazione, simboli obbligatori, conteggio e handle vengono controllati. Le query opzionali di utilizzo, VRAM, temperatura e potenza conservano ciascuna il proprio errore. Una libreria assente, un driver non caricato, zero dispositivi, permessi negati o una funzione non supportata non causano un panic. Gli handle sono richiesti nuovamente a ogni campione; gli UUID identificano i dispositivi quando disponibili, con indice come fallback.

AMD usa solo le interfacce sysfs del driver `amdgpu`: `gpu_busy_percent`, `mem_info_vram_used` e `mem_info_vram_total`. VRAM in byte; nessuna sostituzione con RAM di sistema. Il nome usa `product_name` se presente, altrimenti gli identificatori vendor/device. Altri driver DRM restano esplicitamente non supportati. La potenza AMD rimane `N/D`: sulle APU i valori hwmon possono includere anche la CPU.

I sensori hwmon conservano dispositivo canonico, driver, etichetta del canale, valore in gradi Celsius e limite critico quando disponibile. `coretemp`, `k10temp` e `zenpower` sono classificati come CPU; i driver GPU riconosciuti o associati a un dispositivo DRM sono classificati come GPU. Gli altri sensori mantengono la categoria `Other`. Se hwmon non offre sensori, vengono scoperte le thermal zone, conservando il tipo originale e la categoria `Thermal zone`, senza attribuirle arbitrariamente alla CPU.

Il valore iniziale del pannello termico è il primo sensore CPU, se presente; non è una media né il massimo di tutti i core. `[ ]` seleziona GPU o sensore nel rispettivo pannello. La vista termica dedicata elenca i sensori che entrano nello spazio disponibile; la selezione consente di vedere il valore degli altri canali. I cambi di identità azzerano la curva relativa. Lo storico resta limitato a `history` campioni e interrompe le curve su errori e lacune. Con i default, i grafici CPU/RAM/rete/dischi coprono 120 secondi; GPU e temperatura selezionata coprono 240 secondi.

## Pianificazione e isolamento

Un worker dedicato raccoglie GPU/sensori ogni `hardware_interval` millisecondi: default 2000, limiti 1000–60000. Discovery sysfs e nuovo tentativo di caricamento NVML dopo un errore avvengono ogni 30 secondi. Le letture di device rimossi producono errori locali fino alla discovery successiva. NVIDIA controlla il conteggio a ogni campione.

Lo slot del worker conserva un solo risultato, con lo stesso canale limitato delle notifiche e senza accumulare snapshot. La pausa sospende i campioni successivi e scarta i risultati già in corso. Tastiera e collector CPU/RAM/rete/dischi usano thread indipendenti. Il rendering delle viste GPU/termiche avviene sui loro aggiornamenti e sulle transizioni di obsolescenza, oltre che sull'input/resize. Nella dashboard, se il collector di base è aggiornato e ha un intervallo non superiore a quello hardware, i nuovi dati hardware vengono incorporati nel prossimo frame di base, evitando due ridisegni ravvicinati. Le viste dedicate, la raccolta base più lenta o obsoleta mantengono il rendering diretto degli aggiornamenti hardware.

Una chiamata driver bloccata non viene interrotta forzatamente: può trattenere il worker hardware, ma non i collector di base o l'uscita. Non si avviano thread sostitutivi a ogni scadenza. I dati hardware già ricevuti mostrano `STALE` dopo tre intervalli; prima del primo risultato il pannello mostra lo stato di raccolta. La chiusura non attende il worker bloccato e il sistema recupera i thread alla terminazione del processo.

Non vengono eseguiti subprocess periodici né scritture ai controlli del driver. L'app può funzionare senza libreria NVIDIA, senza directory DRM/hwmon/thermal e senza GPU supportate.

## Diagnostica e benchmark

```bash
cargo build --release --locked
./target/release/rtop doctor
./target/release/rtop doctor --disable-nvml
./target/release/rtop --hardware-interval 2000
./target/release/rtop --benchmark-hardware --runs 3 --warmup 30 --duration 300
```

`doctor` (anche `--doctor`) stampa dispositivi, backend, sensori, metriche e ragioni delle assenze senza inizializzare il terminale. L'assenza di hardware è un risultato diagnostico normale, con uscita 0. `--hardware-sysfs PATH` permette di ispezionare un albero sysfs alternativo o fixture; influenza soltanto il worker hardware. `--disable-nvml` evita il caricamento della libreria.

Il benchmark isolato usa `getrusage(RUSAGE_SELF)` per CPU user+system riferita a un core, `/proc/self/status` per RSS e timestamp monotoni per durata delle raccolte. Il costo GPU comprende discovery/letture DRM/NVML; il costo sensori comprende le letture hwmon/thermal. Riporta separatamente dispositivi DRM, GPU con backend e sensori. I conteggi dispositivi nell'output descrivono l'ultimo campione del run. Il report non trasforma una GPU non supportata in una GPU validata.

## Verifiche di sicurezza e funzionali

```bash
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline
python3 tests/hardware_safety.py
python3 tests/terminal_smoke.py
python3 tests/config_cli.py
```

Quindici test Rust verificano anche sysfs vuoto, DRM non supportato, fixture AMD, contatori fuori limite, rimozione di sensori, fallback termico, libreria/simboli obbligatori assenti, rendering e selezione di dispositivi mancanti, temi/ASCII e stati obsoleti. I precedenti test di collector, configurazione e worker restano attivi.

`tests/hardware_safety.py` compila una libreria NVML fittizia in una directory temporanea, usando `tests/fixtures/nvml.c`. Verifica il contratto ABI e il percorso di errore per zero GPU, inizializzazione fallita, errore di conteggio, handle nullo, permessi negati, query non supportate, GPU persa, valori fuori limite e simboli opzionali mancanti. Avvia inoltre la TUI nei casi senza hardware e con driver non supportati, controllando uscita 0 e ripristino termios/alternate screen. La fixture non viene installata nel sistema.

Con un'inizializzazione NVML fittizia bloccata per 10 secondi, q termina l'app entro il timeout di 2 secondi, senza attendere il driver. L'esito e il tempo effettivo sono in [m4-hardware-safety.json](benchmarks/m4-hardware-safety.json).

## Risultati locali e limiti

Hardware rilevato: Matrox vendor `0x102b`, device `0x0534`; NVML assente; dieci sensori `coretemp`, due package CPU. La dashboard resta operativa e il pannello GPU mostra `N/D`. Il report completo è in [m4-doctor.txt](benchmarks/m4-doctor.txt). L'ambiente CPU/kernel/Rust è quello descritto in [environment.txt](benchmarks/environment.txt).

Tre run hardware da 300 secondi con 30 secondi di riscaldamento ciascuno, più dashboard integrata con tre finestre da 300 secondi su PTY drenato, truecolor 120×40. Collector di base a 1 Hz, hardware a 0,5 Hz. Il costo dell'emulatore grafico resta escluso. I due benchmark sono stati eseguiti contemporaneamente; la CPU è attribuita al singolo processo. Il test di latenza è stato eseguito su un altro processo nella parte iniziale della sessione.

| Misura dashboard | Risultato |
| --- | --- |
| Durata misurata, dopo 30 s di riscaldamento | 1000,004 s (16 min 40 s) |
| CPU media di un core | 0,408% |
| CPU nelle tre finestre da 300 s | 0,410%; 0,410%; 0,403% |
| RSS iniziale / finale | 4224 / 4352 KiB (4,125 / 4,25 MiB) |
| Oscillazione RSS negli ultimi 900 s | 128 KiB; stabile a 4352 KiB nell'ultima finestra |
| Thread massimi | 4 |
| Uscita e ripristino terminale | Verificati, codice 0 |
| Latenza input-render, 40 cambi tema | p50 4,02 ms; p95 4,97 ms; massimo 5,45 ms |
| Uscita durante init NVML fittizia bloccata | 1,15 ms, terminale ripristinato |

La latenza è misurata dopo 125 secondi: storici base pieni e circa 63 campioni termici. Il test non misura il disegno di un emulatore grafico o SSH. La sessione di stabilità comprende anche il periodo con tutti gli storici pieni.

| Run hardware senza rendering | CPU di un core | GPU/discovery media | Sensori media | Raccolta p95 | RSS iniziale / finale |
| --- | --- | --- | --- | --- | --- |
| 1 | 0,0618% | 94 µs | 1726 µs | 2433 µs | 3456 / 3456 KiB |
| 2 | 0,0608% | 84 µs | 1690 µs | 2424 µs | 3456 / 3456 KiB |
| 3 | 0,0618% | 63 µs | 1740 µs | 2362 µs | 3456 / 3456 KiB |

Ogni run comprende 150 campioni, dieci sensori e zero errori dei sensori. GPU rilevate: una DRM non supportata, zero GPU con backend disponibile. CPU media dei collector hardware: 0,0615% di un core. Il costo GPU riportato misura discovery e il percorso senza backend disponibile: non è il costo di NVML/AMDGPU su GPU supportate.

Tutti i controlli locali passano: 15 test Rust, Clippy, formattazione, smoke test del terminale, validazione CLI/TOML e regressioni hardware. Hash SHA-256 release: `e658322a46d78c52fc433322c9f3ce09e31201a3f9a1c7e74ff307970ab092ac`.

Dati grezzi: [hardware CSV](benchmarks/m4-hardware-2s.csv), [hardware log](benchmarks/m4-hardware-2s.log), [dashboard JSON](benchmarks/m4-tui-soak.json), [dashboard CSV](benchmarks/m4-tui-soak.csv), [dashboard log](benchmarks/m4-tui-soak.log), [latenza](benchmarks/m4-input-latency.json), [sicurezza hardware](benchmarks/m4-hardware-safety.json). Anteprime: [scura](previews/m4-live-dark-120x40.svg), [chiara](previews/m4-live-light-120x40.svg).

Riproduzione della sessione integrata e della latenza, con report separati dalla baseline M3:

```bash
python3 tests/tui_soak.py --seconds 1000 --output docs/benchmarks/m4-tui-soak
python3 tests/tui_latency.py --output docs/benchmarks/m4-input-latency.json
```

La breve sessione diagnostica precedente all'accorpamento dei frame è stata interrotta; i report qui collegati sono della build finale.

### Test hardware riportato dall'utente

Output `rtop doctor` ricevuto il 4 ottobre 2026 da un PC Debian con **NVIDIA T1000 8GB**, backend NVML. È un risultato su hardware reale riportato dall'utente, non riprodotto nell'ambiente locale.

| Metrica | Risultato riportato |
| --- | --- |
| Utilizzo | `Ok(0.0)` — 0% nel campione |
| VRAM usata / totale | `425394176 / 8589934592` byte — circa 406 MiB / 8 GiB |
| Temperatura GPU core | `Ok(46.0)` °C, soglia critica non disponibile |
| Potenza | `Err("Not Supported")`, indisponibilità gestita |
| GPU/discovery | 32,680 ms nella singola esecuzione di doctor |
| Sensori separati | 0,000 ms; temperatura GPU già letta tramite NVML |

Il secondo dispositivo DRM (`card0`, vendor/device `0x1234 / 0x1111`) non ha un backend supportato: le query riportano errori locali senza impedire la lettura della T1000. Non viene attribuito un modello a questo dispositivo dal solo output.

La disponibilità di utilizzo, VRAM e temperatura è confermata. Il campione a 0% non verifica la risposta sotto carico; non è ancora disponibile un confronto con uno strumento di riferimento. La durata di doctor include la discovery e non misura il costo dei campioni periodici o il consumo CPU del monitor. Versione Debian, driver NVIDIA, versione/commit del binario, durata del test e benchmark CPU/RAM restano da documentare.

M4 non è segnata come completata: restano la validazione delle metriche e il benchmark NVIDIA, oltre alla validazione delle metriche e del consumo Intel. Il report utente HP ZBook del 6 ottobre 2026 conferma discovery Intel Arc con `i915` e sensori CPU; utilizzo GPU restituisce permessi insufficienti, temperatura GPU e VRAM locale sono indisponibili. Il test AMD reale è in sospeso su richiesta dell'utente. Il backend Intel e le sue verifiche sono documentati in [INTEL-VERIFICATION.md](INTEL-VERIFICATION.md). Le fixture ABI/sysfs verificano la logica e gli errori, ma non sostituiscono questa validazione hardware. Nessun risultato di consumo viene esteso ai futuri collector processi di M5.

## Riferimenti delle interfacce

- [NVIDIA NVML: device queries](https://docs.nvidia.com/deploy/archive/R550/nvml-api/group__nvmlDeviceQueries.html)
- [Kernel AMDGPU: sysfs](https://docs.kernel.org/gpu/amdgpu/index.html)
- [Kernel AMDGPU: temperature e potenza](https://docs.kernel.org/gpu/amdgpu/thermal.html)
- [Kernel hwmon: convenzioni e unità](https://docs.kernel.org/hwmon/sysfs-interface.html)
