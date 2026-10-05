# M5 — Processi reali e navigazione

Implementazione e verifiche del 5 ottobre 2026. I grafici conservano il rendering originale a punti Braille richiesto dall'utente. La verifica hardware M4 resta aperta per confronto/benchmark NVIDIA e test Intel su Fedora; il test AMD reale resta sospeso.

## Metriche e limiti

La raccolta enumera i PID visibili nel mount `/proc` e legge un solo file `PID/stat` per processo. Non legge `cmdline`, `status`, `smaps`, liste di thread o file di dettagli su ogni aggiornamento. Il nome è `comm`, che può essere troncato dal kernel; il parser gestisce spazi, parentesi, byte non UTF-8 e caratteri di controllo. Un processo che termina, un permesso negato o una riga malformata vengono scartati e conteggiati in `skipped`; un errore nell'enumerazione mostra `N/D` senza bloccare la UI. Il mount proc e il namespace PID possono limitare i processi visibili.

CPU% = `100 × delta(utime + stime) / (CLK_TCK × secondi monotoni trascorsi)`; il tempo è misurato per PID fra le sue letture, non assunto pari all'intervallo configurato. La scala è un singolo core: due core occupati possono produrre circa 200%. Non si divide per il numero di CPU e non si sommano i tempi dei figli; il tempo guest già incluso in utime non viene aggiunto. Il primo campione, una ripresa o il riutilizzo di un PID mostrano `...`; un contatore regressivo mostra `reset`. L'identità è `(PID, starttime)`.

RSS = campo `rss` in pagine × `PAGESIZE`, entrambi ottenuti dal kernel; la dimensione di pagina e CLK_TCK vengono letti con `sysconf`. È una stima e include pagine condivise: non viene sommata per costruire la RAM globale e non rappresenta memoria esclusiva. Nessuna lettura costosa di `smaps` viene introdotta per migliorare questa stima. Gli snapshot dei vari PID non sono atomici fra loro.

Definizioni: [documentazione Linux di proc](https://www.kernel.org/doc/html/v6.13/filesystems/proc.html), [proc_pid_stat(5)](https://man7.org/linux/man-pages/man5/proc_pid_stat.5.html).

## Raccolta su richiesta

Un solo worker dedicato, uno slot con l'ultimo frame e notifiche limitate separano la scansione dall'input e dai collector CPU/RAM/GPU. Il worker parte inattivo. Viene attivato nella dashboard completa (almeno 100×32), oppure nella vista processi selezionata quando il terminale ha almeno 60×18. Le viste singole degli altri pannelli, un terminale troppo piccolo, l'aiuto e la pausa lo disattivano. Nessun subprocess periodico viene lanciato.

Il controllo sveglia il worker solo quando cambia la richiesta. Una scansione già iniziata viene annullata tra due letture tramite un contatore atomico; il frame di una richiesta superata viene scartato. Alla riattivazione le basi CPU vengono reinizializzate. Una lettura già bloccata non blocca l'input o il ripristino del terminale. La modalità demo continua a usare fixture; `--live-preview` legge i processi solo se la relativa tabella compare nella cattura.

`process_interval`: default 2000 ms, limiti 1000–60000, configurabile da TOML e `--process-interval`. Nella dashboard gli aggiornamenti processi vengono accorpati al successivo frame base sano quando questo arriva almeno altrettanto spesso; nella vista dedicata vengono mostrati direttamente. I dati oltre tre intervalli sono marcati `STALE`. Lo storico degli altri pannelli e i loro intervalli restano separati.

## Interazione

Selezionare `7` e facoltativamente `Enter` per la vista dedicata. Su/Giù, PgUp/PgDn (15 righe), Home/End navigano i risultati. `s` cicla CPU/RSS/PID/nome; `r` inverte. CPU e RSS iniziano in ordine decrescente, PID e nome crescente. I pareggi hanno ordine PID deterministico e CPU non ancora disponibile viene dopo i valori validi.

`/` avvia un filtro per sottostringa del nome (senza distinzione di maiuscole) o PID. Il risultato si aggiorna durante la digitazione, Enter conclude ed Esc svuota il filtro. Backspace modifica il testo. I normali caratteri, incluso `q`, appartengono al filtro mentre si digita; Ctrl-C resta un'uscita immediata.

Finché non viene navigata una riga, la selezione segue il primo risultato ordinato. Dopo la navigazione segue l'identità del processo attraverso cambi d'ordine; quando questo sparisce seleziona la riga disponibile più vicina. Un PID riutilizzato ha un'identità diversa. La dashboard non selezionata mostra la testa della lista ordinata; nella lista navigata la finestra mantiene visibile la selezione. Filtro e ordinamento vengono ricostruiti su campioni nuovi o modifiche dei controlli, non su ogni redraw. Nessuna azione distruttiva sui processi viene introdotta in M5.

## Verifiche funzionali

22 test Rust, `cargo fmt --check`, Clippy con warning come errori, build debug/release, smoke test del terminale e CLI/TOML superati. I test coprono CPU oltre 100%, PID riutilizzati, uscita durante la lettura, errori, cancellazione, nomi con parentesi/newline, RSS overflow, selezione stabile, filtro, sorting, visibilità e ASCII. Le regressioni hardware NVIDIA/Intel e l'avvio senza GPU supportate passano.

[Process safety](benchmarks/m5-process-safety.json) usa una vera PTY e `inotify` su 64 file stat: zero aperture mentre la tabella è nascosta, in pausa o con aiuto; attivazione dopo cambio vista e resize; filtro/navigazione; uscita mentre una lettura FIFO è bloccata; terminale ripristinato. Le fixture non sostituiscono la verifica delle metriche su proc reale.

## Metodo di misura

Build release SHA-256 `e845dd8dbff680dfe05e0cd03f20de37b3c88e6c45e4491670f7c6797c92f611`.

Il benchmark dedicato usa tre run da 10 s, ciascuno dopo 2 s di warmup, a intervalli di 2 s: cinque scansioni misurate per run. Misura CPU totale del processo con getrusage, costo di raccolta, costruzione della vista e RSS massimo. Quest'ultimo è il picco cumulativo del processo e non una serie di campioni di memoria. Il benchmark non include rendering o GPU e non lancia processi reali per simulare una popolazione elevata.

Le numerosità 100/1000/10000 usano file regolari sintetici su filesystem locale, preparati con lo schema di `tests/process_fixture.py`. Il costo di un mount proc reale con tanti processi può differire. Sono prove di crescita del costo e di reattività, non promesse di consumo per ogni PC. I file vengono lasciati fissi per la misura; le CPU sintetiche risultano zero dopo il campione iniziale.

Esempio riproducibile:

```bash
process_fixture_root=$(mktemp -d)
python3 tests/process_fixture.py "$process_fixture_root/proc-1000" --count 1000
./target/release/rtop --benchmark-processes --process-proc "$process_fixture_root/proc-1000" --warmup 2 --duration 10 --runs 3
python3 tests/process_safety.py
python3 tests/tui_soak.py --seconds 300 --warmup 30 --output docs/benchmarks/m5-tui-soak
python3 tests/tui_latency.py --warmup 125 --output docs/benchmarks/m5-input-latency.json
```

La prova di filtro su 10000 record misura una sequenza di digitazione/applicazione o cancellazione del filtro, seguita da un cambio tema per riconoscere la fine del frame. È una misura della sequenza intera, distinta da un singolo tasto. La latenza dell'emulatore grafico/SSH non è inclusa.

## Risultati e budget

| Popolazione | CPU massima nei 3 run, un core | Raccolta media (range dei run) | RSS massimo | Budget raccolta + vista, 2 s |
| --- | --- | --- | --- | --- |
| Proc reale, 3 PID nell'ultimo campione | 0,0092% | 0,139–0,163 ms | 3456 KiB | Dipende dalla popolazione reale |
| 100 record sintetici | 0,0482% | 0,814–0,929 ms | 3456 KiB | ≤0,1% |
| 1000 record sintetici | 0,4403% | 11,867–16,595 ms | 3840 KiB | ≤1% |
| 10000 record sintetici | 5,3290% | 86,866–112,363 ms | 7816 KiB | ≤6% |

I budget per numerosità sono fissati sulla base di queste misure locali e riguardano il collector più la vista, senza rendering. Il riferimento globale ≤0,5% rimane valido per la dashboard locale con pochi PID, non per qualunque popolazione di processi. Con molti PID si può aumentare `process_interval` per ridurre la frequenza delle scansioni; la latenza dei tasti resta indipendente dalla scansione in corso. Dati grezzi: [proc reale](benchmarks/m5-processes-live.tsv), [100](benchmarks/m5-processes-100.tsv), [1000](benchmarks/m5-processes-1000.tsv), [10000](benchmarks/m5-processes-10000.tsv). Le tre prove sintetiche sono state eseguite in sequenza; altre verifiche locali possono produrre contesa e non si deduce un miglioramento rispetto alle build precedenti.

[Verifica CPU su processo reale](benchmarks/m5-process-live-verification.json): un figlio impegnato in un loop CPU produce 98,7% nella tabella e 99,35% nella misura indipendente dei tick/tempo attorno alla cattura, entro la tolleranza di 15 punti percentuali necessaria per i diversi istanti di lettura. RSS positivo 8,9 MiB. La prova con CPU oltre 100% resta una fixture a intervallo controllato, non un test reale multicore.

| Dashboard reale su PTY 120×40, base 1 Hz e processi 2 s | Risultato |
| --- | --- |
| Durata misurata dopo 30 s di warmup | 300 s |
| CPU media di un core | 0,277% |
| RSS iniziale / finale | 4352 / 4480 KiB |
| Variazione RSS nella finestra | 128 KiB |
| Thread massimi | 5 |
| Latenza p95 su 40 input, warmup 125 s | 2,72 ms |
| Ripristino del terminale | Verificato |

La dashboard include processi reali nel namespace ristretto del sandbox, CPU/RAM/rete/dischi e sensori CPU; nessuna GPU supportata è presente. Il costo dell'emulatore grafico, di SSH e delle GPU supportate reali è escluso. Il benchmark separato proc reale osserva 3 PID; la popolazione delle diverse prove può variare e non rappresenta un desktop completo. Dati: [soak JSON](benchmarks/m5-tui-soak.json), [CSV](benchmarks/m5-tui-soak.csv), [latenza](benchmarks/m5-input-latency.json).

Con 10000 record sintetici, [cambio tema](benchmarks/m5-large-input-latency.json) p95 7,97 ms; [sequenza filtro completo/clear + tema](benchmarks/m5-large-filter-latency.json) p95 47,86 ms, massimo 56,22 ms. Entrambe le prove usano 40 input e warmup 3 s. Budget di reattività: p95 ≤100 ms. I vincoli raccolta/vista e input sono stati verificati sui report; la pausa e la vista nascosta richiedono invece zero aperture stat, verificato tramite inotify.

M5 soddisfa i criteri di completamento locali. La sessione prolungata di 8 ore, ulteriori terminali e la matrice hardware restano parte di M6 o della validazione hardware M4.
