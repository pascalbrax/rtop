# M3 — Dashboard con dati reali

Completata e verificata il 4 ottobre 2026, versione di sviluppo `0.1.0`.

## Funzionamento

L'avvio normale mostra CPU, RAM, rete e dischi reali. GPU, temperature e processi conservano le fixture del prototipo e l'etichetta `DEMO` sul singolo pannello. `--demo` riproduce il prototipo originale; le anteprime sono deterministiche salvo `--live-preview`.

Un worker raccoglie i dati senza bloccare la tastiera. Uno slot condiviso conserva solo l'ultimo snapshot; un canale da 32 elementi trasporta input e notifiche, senza code di payload storici. Se il canale è pieno, lo snapshot nello slot viene comunque sostituito. Il thread input attende gli eventi senza polling periodico.

I filesystem sono condivisi tramite `Arc`: i dati vengono aggiornati al proprio intervallo senza copiare l'elenco a ogni campione. Il worker attende tramite condvar e reagisce immediatamente a pausa, ripresa e uscita. La pausa sospende i campionamenti successivi, scarta eventuali risultati in corso e azzera le basi dei contatori alla ripresa. Al primo campione successivo le metriche basate su differenze di contatori sono `collecting`; la RAM è subito disponibile. La UI non inventa velocità attraverso la pausa.

L'input reader bloccante non viene atteso all'uscita: il processo termina e il sistema recupera il thread. Non è un servizio riutilizzabile all'interno di un processo ospite. Anche il worker non viene atteso se una chiamata di raccolta è bloccata; q/Ctrl-C resta disponibile.

## Visualizzazione e storico

- Layout approvato: CPU/GPU sopra, RAM/processi affiancati, dischi/rete/temperature sotto.
- Errori localizzati nel pannello; metriche assenti mostrano `N/D` e i contatori iniziali/reset hanno uno stato distinto.
- Un campione non aggiornato per oltre tre intervalli segnala `STALE`. Durante la pausa il tempo visualizzato resta congelato.
- Storico di 120 campioni per serie per default, con buffer limitati; capacità configurabile tra 10 e 3600.
- Ascissa basata sui timestamp monotoni, con finestra `history × interval`.
- Curve interrotte su errori, reset e lacune superiori a 1,8 intervalli; i campioni fuori finestra vengono esclusi.
- Cambio dispositivo o interfaccia azzera la relativa serie per evitare di unire fonti diverse.
- CPU in percentuale, RAM in GiB, I/O e rete in KiB/s o MiB/s; R/W e RX/TX sono curve separate. Le barre percentuali indicano soltanto quantità con un totale noto.
- Default rete: prima interfaccia diversa da loopback, oppure loopback. Default filesystem: root. Default I/O: dispositivo che sostiene il filesystem selezionato, quando presente.
- `[ ]` cambia disco o interfaccia nel rispettivo pannello; `f` cambia filesystem nel pannello dischi. Un selettore esplicito non disponibile produce `N/D`.

Il rendering parte da input che cambia la vista, resize, scadenza di obsolescenza o campione che interessa la vista visibile. Le viste demo dedicate e l'aiuto non sono ridisegnati a ogni campione reale. Ratatui emette solo le differenze rispetto al frame precedente.

## Configurazione

Esempio in [config/example.toml](../config/example.toml). File predefinito: `$XDG_CONFIG_HOME/rtop/config.toml` oppure `~/.config/rtop/config.toml`. Non viene creato automaticamente. `--config PATH` richiede che il file esista.

Ordine: valori predefiniti, file TOML, `NO_COLOR`, argomenti CLI espliciti. `--no-color=false` forza i colori anche in presenza di `NO_COLOR`. Il tasto `c` aggiorna sia la palette sia l'emissione ANSI dei colori.

Chiavi sconosciute, tipi errati, intervalli e capacità fuori limite vengono rifiutati prima di entrare nel terminale. L'intervallo filesystem è una soglia minima: la lettura avviene al campione successivo del worker. Headless e benchmark rispettano la stessa configurazione degli intervalli.

## Verifiche funzionali

- Test Rust: parsing dei collector, recupero da errori/hotplug, configurazione, timestamp e lacune, sostituzione snapshot a canale pieno, pausa/ripresa e rendering con dati reali/errori/staleness.
- `cargo clippy --offline --all-targets -- -D warnings`, build release e formattazione.
- `tests/terminal_smoke.py`: TUI live e demo, pausa senza output periodico, resize, tastiera, q/Ctrl-C e ripristino termios/alternate screen.
- `tests/config_cli.py`: caricamento TOML, precedenza CLI, override NO_COLOR e rifiuto degli errori prima dell'inizializzazione terminale.
- `tests/tui_latency.py`: 40 cambi tema con collector attivi, dopo 125 secondi per riempire gli storici; misura dall'invio del tasto alla palette attesa seguita dal marker di fine frame. Obiettivo p95 ≤100 ms.

La latenza e il rendering sono misurati su uno pseudo-terminale drenato. Non comprendono il tempo di disegno di un emulatore grafico o la latenza di SSH.

## Protocollo di stabilità e prestazioni

```bash
cargo build --release
python3 tests/tui_latency.py
python3 tests/tui_soak.py
```

Il test prolungato avvia la TUI release a 1 Hz, tema scuro truecolor, 120×40, capacità storico 120, con 30 secondi iniziali di riscaldamento e 3600 secondi misurati. Drena l'output dello pseudo-terminale senza registrare ogni frame su disco.

All'interno della sessione misura tre finestre da cinque minuti, ciascuna preceduta da 30 secondi di riscaldamento. CPU ricavata dai tick user+system aggregati del processo in `/proc/PID/stat`, includendo tutti i thread, divisa per il tempo monotono realmente trascorso e riferita a un core. Risoluzione registrata nel report; non è divisa per il numero dei core. Il costo del monitor Python non è attribuito al processo rtop.

RSS e numero di thread vengono campionati ogni minuto. Il test verifica uscita e ripristino del terminale, budget CPU ≤0,5% e assenza di crescita continua della memoria nella parte finale. Lo slot snapshot e i buffer storici hanno limiti strutturali; il test a canale pieno verifica che rimanga il campione più recente.

L'ambiente resta il sandbox Linux di M2: rete loopback, contatori kernel esposti dall'host/container, senza normalizzazione cgroup. GPU e processi sono demo statiche e non aggiungono collector reali. L'emulatore grafico resta fuori dalla misura. Il risultato non costituisce una misura dei futuri backend GPU/processi.

Il report contiene l'hash del binario e i namespace. Dati: [CSV](benchmarks/m3-tui-soak.csv), [report JSON](benchmarks/m3-tui-soak.json), [log](benchmarks/m3-tui-soak.log), [latenza](benchmarks/m3-input-latency.json).

## Risultati

Tutti i criteri M3 sono superati. Dodici test Rust, smoke test terminale, verifiche CLI/TOML, Clippy e formattazione passano.

| Misura | Risultato |
| --- | --- |
| Sessione misurata | 3600,000 s, dopo 30 s di riscaldamento |
| CPU media, riferita a un core | 0,330% (obiettivo ≤0,5%) |
| RSS iniziale / finale | 3840 / 3840 KiB (3,75 MiB) |
| Oscillazione RSS negli ultimi 15 minuti | 0 KiB |
| Thread massimi | 3 |
| Ripristino terminale e uscita | Verificati, codice 0 |
| Latenza input-render, 40 campioni | p50 4,20 ms; p95 5,57 ms; massimo 6,01 ms |

Le tre finestre da circa 300 secondi, ciascuna con 30 secondi precedenti di riscaldamento, confermano il budget:

| Finestra | CPU di un core | RSS iniziale / finale |
| --- | --- | --- |
| 1 | 0,327% | 3840 / 3840 KiB |
| 2 | 0,340% | 3840 / 3840 KiB |
| 3 | 0,320% | 3840 / 3840 KiB |

Hash SHA-256 del binario misurato: `509e6638339f2da3a7db0e8aca5e7baf879cf56d3b412e347ff24d6969ea440f`. I tick CPU hanno risoluzione 10 ms. I dati grezzi e le condizioni della misura sono nei report collegati sopra.

La pausa reimposta le basi delle velocità; lo storico RAM mantiene i campioni validi e interrompe la curva quando la lacuna supera 1,8 intervalli. Le prove brevi `m3-tui-probe.*` erano diagnostiche con `NO_COLOR` ereditato e sono superate dalla sessione truecolor qui riportata.

Anteprime: [tema scuro](previews/live-dark-120x40.svg), [tema chiaro](previews/live-light-120x40.svg). GPU, temperature e processi restano esplicitamente `DEMO`, rispettivamente in attesa di M4 e M5.
