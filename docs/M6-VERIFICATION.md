# M6 — candidato Linux e verifica

Avviata il 7 ottobre 2026 sulla versione `0.1.0-rc.1`. La milestone è **in corso**: la sessione di stabilità di 8 ore è stata avviata, ma non ha ancora prodotto un esito finale. Nessun tag di release finale è stato creato. M4 conserva la validazione hardware aperta; AMD reale resta sospesa.

## Modifiche e controlli

Il rendering e la palette conservano i grafici originali a dots Braille e i colori distinti dei pannelli. Un test misura contrasto testo/sfondo ≥4,5:1 su testo principale, secondario e colori delle sezioni, nei due temi; verifica inoltre che la modalità senza colore usi colori di reset. Non si attribuisce questo rapporto alla griglia decorativa o ai segmenti spenti degli indicatori. Layout 60×18, 80×24, 120×40 e 160×50, ASCII, aiuto, errori e stati obsoleti sono coperti dai test di rendering.

L'avvio interattivo controlla stdin e stdout prima di modificare il terminale: una redirezione produce un errore leggibile senza panic e senza escape di apertura della TUI. `doctor`, raccolta e anteprime continuano a funzionare senza TTY. La guardia di ripristino viene installata prima di `try_init`, per coprire anche eventuali errori di inizializzazione parziale. Nessun polling aggiuntivo, thread o collector viene introdotto.

23 test Rust, formattazione e Clippy con warning come errori superati. Smoke test terminale e CLI/TOML superati. Fixture hardware NVIDIA/Intel e regressioni processi superate: backend assenti/non supportati, errori/permessi, init lenta, FIFO bloccata, pausa, vista nascosta, filtro e ripristino terminale. L'uscita con init lenta è circa 3,3 ms nel controllo locale; il test FIFO include un drain di 100 ms. Queste misure verificano reattività agli errori, non validazione delle metriche di GPU reali.

Report: [smoke](benchmarks/m6-terminal-smoke.log), [configurazione](benchmarks/m6-config-cli.log), [processi](benchmarks/m6-process-safety.json), [Intel](benchmarks/m6-intel-safety.json), [hardware](benchmarks/m6-hardware-safety.json).

Il test [terminal_matrix.py](../tests/terminal_matrix.py) usa una PTY drenata con profili `xterm-256color`, `screen-256color`, `linux`, `vt100` e `dumb` (fallback ASCII/senza colore per gli ultimi tre), 21 resize per profilo, pausa, sezioni, aiuto e uscita q/Ctrl-C. Sono verifiche di protocollo; il 7 ottobre l’utente ha riferito una prova d’uso reale su Gentoo via SSH con Windows Terminal. Versioni del client SSH/Windows Terminal e risultati specifici di tema chiaro/scuro, ASCII, resize e Ctrl-C non sono forniti: la prova d’uso viene registrata senza attribuirle tutti gli esiti dei test automatici. Tutti e cinque i profili superano il controllo, compreso ripristino del terminale e assenza di SGR colorati in modalità senza colore. Report: [matrice terminali](benchmarks/m6-terminal-matrix.json).

## Profilo dei costi e prestazioni

Ambiente locale: Gentoo Linux 2.18, kernel `6.8.12-13-pve`, Xeon E5-2643, circa 126 GiB di RAM; namespace PID ristretto. Toolchain Rust/Cargo 1.95.0. GPU Matrox non supportata e dieci sensori CPU; nessuna GPU NVIDIA/AMD/Intel supportata disponibile. Profilo dell'emulatore grafico e latenza SSH esclusi.

Sono misurati separatamente collector base, hardware/sensori e processi con gli strumenti headless esistenti, che riportano il costo dei gruppi e getrusage. Le misure M6 brevi usano 3 run da 10 s, ciascuno dopo 2 s di warmup: sono un controllo dei costi, non sostituiscono il protocollo esteso M2/M4 e non provano un miglioramento. Sono eseguite in sequenza mentre altre verifiche e il soak possono essere attivi; non viene presentato un confronto senza contesa. `perf` e `strace` non sono disponibili nell'ambiente: nessun flamegraph o profilo di stack viene dichiarato.

| Profilo separato | CPU nei tre run, un core | Costi osservati |
| --- | --- | --- |
| Collector base, 1 Hz | 0,1133–0,1364% | Raccolta p95 1,525–1,619 ms; I/O dischi 0,601–0,705 ms medi, gruppo più costoso |
| Hardware/sensori, 2 s | 0,0405–0,0541% | Sensori 0,869–1,197 ms medi; nessuna GPU supportata |
| Collector/vista processi, 2 s, 3 PID | 0,0079–0,0117% | Raccolta media 0,138–0,201 ms; vista 0,0024–0,0029 ms |

I gruppi base non riportano errori o scadenze saltate; le finestre sono troppo brevi per trarre conclusioni sul consumo continuativo. I valori separati non vengono sommati come se fossero una misura della dashboard. Il gruppo disco è il maggiore costo di raccolta base osservato, ma il budget è rispettato senza modificare i collector. La latenza input-render su 40 cambi tema, dopo 125 s di warmup, è p95 **2,54 ms**, massimo **4,08 ms**, entro 100 ms.

Dati: [base CSV](benchmarks/m6-base-profile.csv), [hardware CSV](benchmarks/m6-hardware-profile.csv), [processi TSV](benchmarks/m6-process-profile.tsv), [latenza](benchmarks/m6-input-latency.json), [confronto CPU/RSS processo reale](benchmarks/m6-process-live-verification.json). I budget e la crescita del costo con 100/1000/10000 processi restano documentati in [M5-VERIFICATION.md](M5-VERIFICATION.md); i collector non sono modificati da M6. Non viene introdotta un'ottimizzazione senza un costo dimostrato da correggere.

Anteprime deterministiche del candidato: [scura](previews/m6-dark-120x40.svg), [chiara](previews/m6-light-120x40.svg), [ASCII senza colore](previews/m6-ascii-80x24.txt).

## Regressione breve

Sessione locale da 120 s misurati dopo 30 s di warmup sulla stessa build del candidato: CPU **0,3667%** di un core, RSS 4224→4480 KiB, range 256 KiB, massimo 5 thread, uscita 0 e terminale ripristinato. Tutti gli assert passano; non sostituisce la sessione di 8 ore. Dati: [JSON](benchmarks/m6-short-soak.json), [CSV](benchmarks/m6-short-soak.csv), [stato passato](benchmarks/m6-short-soak.status.json).

## Sessione di 8 ore

Il runner è `tests/tui_soak.py --seconds 28800 --warmup 30`, PTY 120×40 truecolor, dashboard reale a 1 Hz, hardware/processi a 2 s. La copia del binario sotto `target/m6-soak/rtop` conserva l'eseguibile della sessione anche se una build successiva aggiorna `target/release/rtop`. Lo SHA della build finale misurata è `5762ad981a8b85de9b56f219bc8dbe02957d63ca5a69bd1379a2d0dde2c7b9e2`, registrato anche nel report di stato. La prima prova preliminare è stata interrotta dopo un rebuild per ripartire con la stessa build del candidato; non conta nella durata richiesta. I PID del report appartengono al namespace della prova.

Stato **in corso**. I campioni CSV vengono scritti ogni minuto; lo stato JSON distingue `running`, `passed` e `failed`. La prova deve completare almeno 28800 s misurati dopo il warmup, senza uscita anticipata, CPU media e finestre ≤0,5% di un core, range RSS finale ≤256 KiB, range completo ≤1 MiB, massimo 5 thread, uscita 0 e ripristino terminale. Un JSON delle misure non basta: anche gli assert devono passare e lo stato deve risultare `passed`. Le altre verifiche durante l'inizio della sessione possono influire sulla contesa; il consumo è attribuito al processo rtop.

```bash
cat docs/benchmarks/m6-tui-soak.status.json
# Per riprodurre dall'inizio:
python3 tests/tui_soak.py --seconds 28800 --warmup 30 --output docs/benchmarks/m6-tui-soak
```

Non vengono dichiarati consumo medio, stabilità di 8 ore o ripristino finale prima che la sessione termini. I risultati delle sessioni precedenti restano storici, senza estenderli alla durata di questa prova.

## Distribuzione e CI

Versione candidata `0.1.0-rc.1`, licenza MIT aggiunta al repository, `publish = false`, metadati del repository e script `scripts/package_release.py`. Il candidato Linux contiene binario, note, configurazione, inventario e testi di licenza delle dipendenze host risolte. Archivio e checksum sono generati in `dist/` e ignorati da Git. Toolchain, target, libc, dipendenze dinamiche e SHA del binario sono registrati in `BUILD-INFO.json`.

La CI Linux è configurata con azioni fissate a SHA ufficiali, permessi di lettura e credenziali Git non persistenti; esegue controlli, fixture, una sessione breve e packaging, caricando un artifact. Non pubblica tag o GitHub Releases. La [CI remota sul candidato `73a6e29`](https://github.com/pascalbrax/rtop/actions/runs/37689198595) è conclusa con successo: controlli Rust, regressioni, stabilità breve, packaging e upload artifact. Evidenza: [stato CI e artifact](benchmarks/m6-ci.json). Questo non chiude la sessione locale di 8 ore. Il pacchetto locale GNU x86_64 è costruito con glibc 2.43; non si estende questa compatibilità ad altri host. Contiene 75 dipendenze host risolte con inventario e testi di licenza. Due packaging consecutivi degli stessi contenuti e della stessa build producono un checksum archivio identico; gli indirizzi ASLR di `ldd` sono esclusi dai metadati per non introdurre differenze spurie. La verifica di estrazione, checksum archivio/binario, versione e avvio del binario estratto (`doctor` senza GPU e raccolta headless) è superata. Report: [pacchetto](benchmarks/m6-package-verification.json).

Nome già usato da altri monitor; il repository `pascalbrax/rtop` resta l'identità della distribuzione, senza pubblicazione omonima su crates.io. Licenze dei crate e notice vengono incluse senza distribuire driver proprietari opzionali. Metodo e installazione: [RELEASE.md](RELEASE.md).

## Attività ancora aperte

- Esito della sessione reale di almeno 8 ore e revisione del report finale.
- Dettagli delle verifiche visuali su Windows Terminal/SSH (prova d’uso riferita dall’utente, versioni e singoli esiti non specificati); CI remota del candidato già superata.
- Validazione/benchmark hardware M4, compresi i contatori Intel Arc mancanti sullo ZBook; AMD sospesa.
- Tag e release finale dopo conclusione delle verifiche richieste.
