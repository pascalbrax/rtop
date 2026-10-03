# M2 — Collector Linux e baseline delle prestazioni

## Ambito

Implementati collector reali e un'interfaccia headless, indipendenti dalla TUI approvata. La TUI e il pannello processi restano simulati fino alle milestone di integrazione. Nessun subprocess viene avviato dai collector; gli strumenti esterni sono usati esclusivamente nelle verifiche.

- CPU totale e per core da `/proc/stat`; load average da `/proc/loadavg`.
- RAM, memoria disponibile, cache, buffer e swap da `/proc/meminfo`.
- Byte RX/TX e velocità per interfaccia da `/proc/net/dev`, con ifindex dove disponibile.
- Byte letti/scritti e velocità per dispositivo da `/proc/diskstats`, identificato da major:minor e nome.
- Spazio dei filesystem locali da mountinfo e `statvfs`, aggiornato ogni 30 secondi.

Ogni gruppo conserva timestamp monotono, durata della raccolta e risultato o errore. Le velocità distinguono raccolta iniziale, reset, assenza di progresso e valore valido. Dopo rimozione e ricomparsa di un dispositivo, la velocità torna in raccolta iniziale. Un errore di una fonte non interrompe gli altri collector.

## Definizioni

CPU occupata = differenza dei contatori non idle / differenza totale × 100. `iowait` conta come idle; guest/guest_nice non vengono sommati nuovamente, poiché sono già inclusi in user/nice. Il decremento documentato di iowait viene limitato a zero per quel campo. I decrementi degli altri contatori indicano reset.

Memoria utilizzata = totale − disponibile. Cache = Cached + SReclaimable − Shmem, limitata a zero; Buffers è separato. Le unità `kB` di meminfo sono convertite in byte mediante ×1024. È richiesto MemAvailable; la sua assenza produce errore esplicito senza una stima silenziosa.

I settori diskstats sono convertiti con ×512, indipendentemente dalla dimensione fisica del settore. Rete e disco usano il tempo monotono trascorso tra letture riuscite della rispettiva fonte. Non viene assunto che l'intervallo reale sia esattamente un secondo.

Filesystem: totale = f_blocks × f_frsize; libero = f_bfree × f_frsize; disponibile agli utenti = f_bavail × f_frsize; utilizzato = totale − libero. Libero e disponibile restano distinti. I mount sono deduplicati per major:minor e tipo, conservando il primo mount incontrato. Subvolume e bind mount dello stesso filesystem non generano totali duplicati.

I/O di dispositivi fisici, partizioni e volumi virtuali resta separato e non viene sommato attraverso questi livelli. Neppure le interfacce fisiche e virtuali vengono sommate indiscriminatamente. La classificazione dei dischi è riscoperta ogni 30 secondi; i contatori sono letti a ogni campione. La rete riscopre le interfacce a ogni lettura.

La raccolta spazio supporta ext2/3/4, xfs, btrfs, zfs, vfat, exfat, ntfs3 e overlay; mount di rete, FUSE e filesystem pseudo non sono interrogati. Un errore `statvfs` viene conservato per il singolo mount. L'età della cache filesystem è esposta nell'output headless.

## Comandi

```bash
cargo build --release
./target/release/rtop --collect 3 --interval 1000
./target/release/rtop --benchmark --runs 3 --warmup 30 --duration 300 --interval 1000
```

`--collect` scrive TSV con una riga `sample` per campione e righe per le metriche. Unità: byte, byte/s e percentuale CPU. I filesystem includono l'età del dato. `sampling`, `reset` e `no-progress` sono stati, non valori zero.

Campi TSV:

| Riga | Campi dopo il tipo |
| --- | --- |
| sample | indice, secondi monotoni dall'avvio |
| cpu | ID, percentuale/stato |
| memory | totale, disponibile, usata, cache, buffer, swap totale, swap usata |
| load | medie a 1, 5 e 15 minuti |
| network | nome, ifindex, RX totale, TX totale, RX/s, TX/s |
| disk | major:minor, nome, tipo, letture totali, scritture totali, letture/s, scritture/s |
| filesystem | dispositivo, mount, tipo, totale, usato, libero, disponibile, età |
| error | fonte, descrizione |

## Correttezza verificata

`cargo test --offline`: 7 test superati. Fixture coprono parsing, conversioni, guest CPU, diminuzione iowait, reset, intervallo irregolare di 2,5 secondi, unità errate, dati malformati, hotplug, recupero dopo errore, deduplica mount ed esclusione NFS. Restano presenti i test del prototipo visivo.

`cargo clippy --offline --all-targets -- -D warnings` e build release superati.

`tests/verify_collectors.py` confronta il binario release con fonti indipendenti:

- RAM vs `free -b`, con tolleranza 1% della memoria totale per variazioni durante le letture.
- CPU vs due intervalli da un secondo di `vmstat`, escludendo la media dall'avvio. Le finestre sono quasi allineate; tolleranza di 5 punti percentuali sulla media.
- Contatori rete compresi tra le letture prima/dopo di `ip -j -s link`.
- Contatori disco compresi tra le letture prima/dopo delle statistiche sysfs.
- Spazio della root vs `df -B1`, con tolleranza 0,1% sullo spazio variabile.
- Velocità coerenti con le differenze dei contatori esportati e gli intervalli monotoni, con tolleranza 1% per i timestamp specifici dei collector.

Il confronto live è superato: CPU rtop 29,629630% e 27,226463%; vmstat 30% e 27%; 6 righe rete e 102 righe disco verificate. Campioni conservati in [live-samples.tsv](benchmarks/live-samples.tsv).

## Protocollo del benchmark

Scenario M2: collector CPU/RAM/load/rete/I/O a 1 Hz, spazio filesystem ogni 30 secondi, senza TUI, GPU o processi. Tre run con 30 secondi di riscaldamento e 300 secondi misurati ciascuno. Build release. Il processo attende la prossima scadenza; in caso di ritardo evita raffiche di recupero.

CPU = differenza user+system del processo da `getrusage` / tempo monotono trascorso ×100. Il valore si riferisce a un singolo core e non è diviso per il numero dei core. L'overhead della strumentazione è incluso. CSV contiene quantili e massimo del tempo di raccolta, medie per fonte, errori, RSS iniziale/finale e massimo RSS, context switch e scadenze saltate.

`timer_wakeups` conta le attese pianificate del ciclo, non tutti i risvegli del kernel. I context switch volontari/involontari sono riportati separatamente; non vengono presentati come una misura diretta di tutti i wakeup. Le medie per fonte hanno risoluzione al microsecondo; possono differire dalla durata totale, che comprende coordinamento e allocazioni.

L'ambiente è documentato in [environment.txt](benchmarks/environment.txt), incluso hash SHA256 del binario. Il benchmark è eseguito nel sandbox, con sola interfaccia loopback visibile. Il confronto live è eseguito fuori dal sandbox nel namespace di `ip`, con eth0 e lo. I due ambienti non vengono confusi.

I contatori riflettono le risorse esposte da `/proc` e `/sys`, che nei container possono appartenere al sistema host. Non è implementata la normalizzazione ai limiti cgroup. GPU e driver GPU non sono parte di questa misura. Una release su hardware desktop dovrà essere misurata nello stesso ambiente in cui viene eseguita la TUI.

## Risultati

| Run | CPU di un core | RSS iniziale/finale | Raccolta p50 | Raccolta p95 | Massimo |
| --- | --- | --- | --- | --- | --- |
| 1 | 0.123% | 2944/2944 KiB | 1125 µs | 1988 µs | 3363 µs |
| 2 | 0.129% | 2944/2944 KiB | 1262 µs | 1934 µs | 3445 µs |
| 3 | 0.138% | 2944/2944 KiB | 1284 µs | 2037 µs | 4570 µs |

Media CPU dei tre run: **0.130% di un core**. Totale: 900 campioni, zero errori e zero scadenze saltate. RSS stabile a 2944 KiB nei punti osservati. Circa un'attesa pianificata al secondo e 300 context switch volontari per run; 18, 13 e 16 involontari rispettivamente.

Diagnostica separata dopo la baseline: un run di 10 secondi a 10 Hz, con 1 secondo di riscaldamento, in [m2-diagnostic-10hz.csv](benchmarks/m2-diagnostic-10hz.csv). È una misura breve dei costi per fonte, non una seconda baseline conforme al protocollo dei tre run. La raccolta I/O è la fonte più costosa: media 653 µs; CPU 273 µs, rete 195 µs, memoria 77 µs e load 34 µs. RSS 2816 KiB, zero errori. Non sono necessari privilegi perf né campionamento del profiler nel benchmark principale.

Il confronto live e la baseline hanno hardware/kernel comuni ma namespace diversi, documentati sopra. Il risultato non estende il budget a GPU, processi, altre macchine o dashboard con rendering.

Dati grezzi: [CSV](benchmarks/m2-collectors-1hz.csv) e [log](benchmarks/m2-collectors-1hz.log).

L'obiettivo ≤0,5% di un core per la dashboard completa non viene attestato da questo scenario: la TUI reale verrà misurata in M3.

## Riferimenti

- [Linux /proc: CPU e memoria](https://docs.kernel.org/filesystems/proc.html)
- [Linux statistiche I/O: contatori e settori](https://docs.kernel.org/admin-guide/iostats.html)
- [Linux statistiche delle interfacce](https://docs.kernel.org/networking/statistics.html)
- [statvfs: unità e spazio disponibile](https://man7.org/linux/man-pages/man3/statvfs.3.html)
