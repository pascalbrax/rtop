# Milestone

**M1 e M2 completate il 3 ottobre 2026** (`rtop 0.1.0`); **M3 completata il 4 ottobre 2026**; M4 è implementata e in validazione hardware; M5 completata il 5 ottobre 2026; M6 è da avviare. Risultati M2: [collector e benchmark](docs/M2-VERIFICATION.md). Evidenze e limiti: [verifica M1](docs/M1-VERIFICATION.md). Questo documento definisce risultati e criteri di completamento. Visione e vincoli sono in [ROADMAP.md](ROADMAP.md).

## M1 — Design e prototipo visivo

**Risultato:** prototipo Ratatui navigabile con dati simulati deterministici.

- [x] Creare il progetto Rust, CLI minima ed event loop.
- [x] Definire palette, spaziature, formati numerici e componenti condivisi.
- [x] Disegnare dashboard e viste dedicate per tutte le categorie.
- [x] Implementare temi chiaro/scuro, fallback ASCII e modalità senza colore.
- [x] Gestire tastiera, ridimensionamento e ripristino del terminale.
- [x] Verificare layout a 80×24, 120×40 e 160×50; sotto la dimensione minima mostrare un messaggio leggibile.

**Completamento:** tutte le sezioni sono accessibili da tastiera; nessuna sovrapposizione alle dimensioni previste; valori lunghi e metriche mancanti non rompono il layout. Conservare screenshot rappresentativi e test di rendering mirati con `TestBackend`.

## M2 — Collector essenziali e benchmark

**Dipendenza:** M1.

**Risultato:** raccolta Linux di CPU, RAM, rete e dischi, con misure ripetibili indipendenti dalla UI.

- [x] Definire snapshot, stati delle metriche, identificatori e timestamp monotoni.
- [x] Implementare collector mirati e pianificazione delle frequenze.
- [x] Gestire primo campione, reset dei contatori, device rimossi e errori di lettura.
- [x] Verificare parsing e velocità con fixture e intervalli irregolari.
- [x] Confrontare i dati con strumenti di sistema usando finestre e definizioni equivalenti.
- [x] Documentare CPU, RAM, kernel, terminale, hardware, driver e versione del binario di riferimento.
- [x] Definire benchmark: build release, 30 s di riscaldamento e almeno tre misure da 5 minuti per scenario.
- [x] Registrare consumo CPU, durata dei collector, risvegli e memoria; usare profiling separato per non alterare il benchmark principale.

**Completamento:** metriche corrette nei casi verificati e baseline riproducibile. Se il budget preliminare non è raggiungibile, documentare cause e interventi prima di ampliare il carico.

## M3 — Dashboard con dati reali

**Dipendenza:** M2.

**Risultato:** monitor usabile per CPU, RAM, dischi e rete.

- [x] Collegare collector e UI con canale limitato e politica dell'ultimo snapshot.q
- [x] Implementare storico circolare di 120 campioni e grafici basati sui timestamp.
- [x] Disegnare solo quando input, resize o dati visibili lo richiedono.
- [x] Implementare selezione pannelli, pausa, aiuto e layout adattivo.
- [x] Aggiungere CLI e configurazione TOML con validazione degli intervalli.
- [x] Mostrare stati di raccolta, dati obsoleti ed errori localizzati.
- [x] Misurare la dashboard integrata a 1 Hz.

**Completamento:** dashboard stabile durante una sessione di almeno un'ora, nessuna crescita continua di memoria o code, consumo medio entro l'obiettivo ≤0,5% di un core sulla macchina di riferimento. Obiettivo di latenza input-render: p95 ≤100 ms nelle condizioni documentate.

## M4 — GPU e temperature

**Dipendenza:** M3.

**Risultato:** pannelli GPU e sensori con supporto esplicito delle capacità disponibili.

- [x] Implementare discovery e backend NVIDIA via NVML caricato a runtime.
- [x] Implementare backend AMD tramite interfacce disponibili del driver.
- [x] Scoprire sensori CPU/GPU preservando dispositivo, etichette e tipo di temperatura.
- [x] Integrare thermal zones come fallback senza attribuzioni arbitrarie alla CPU.
- [x] Gestire librerie assenti, GPU non supportate, permessi mancanti e dispositivi rimossi.
- [x] Implementare `doctor` con backend, dispositivi e ragioni delle metriche mancanti.
- [x] Aggiungere discovery Intel `i915`/`xe` e backend opzionale Level Zero Sysman.
- [x] Confermare su Debian discovery e disponibilità di utilizzo, VRAM e temperatura NVIDIA T1000 8GB; potenza non supportata gestita.
- [ ] Completare confronto delle metriche sotto carico e benchmark NVIDIA T1000 su Debian; registrare versioni driver.
- [x] Verificare discovery Intel su hardware reale: HP ZBook, Intel Arc `8086:7d55`, driver `i915` (report utente 6 ottobre 2026).
- [ ] Completare validazione delle metriche Intel Arc; verificare i gruppi engine disponibili e risolvere o documentare i limiti di accesso (Fedora 44, kernel 7.1.13, Arc Meteor Lake-P identificati).
- [ ] Validare AMD reale e registrarne driver/modello — **in sospeso su richiesta dell'utente**.
- [ ] Misurare il costo aggiuntivo di ogni backend GPU e dei sensori.

**Completamento:** validazione hardware documentata, nessun subprocess periodico e nessun blocco della UI durante errori del backend. Se l'hardware non è disponibile, lasciare la relativa validazione aperta: i test simulati non la sostituiscono. Pubblicare consumo e limiti della dashboard completa.

**Verifica locale M4:** implementazione e test senza GPU disponibili in [M4-VERIFICATION.md](docs/M4-VERIFICATION.md). 15 test Rust e regressioni hardware passano; sessione integrata da 1000 s: CPU 0,408% di un core, RSS finale 4352 KiB, p95 input-render 4,97 ms. Sensori e percorso senza backend misurati in tre run da 300 s. Discovery e lettura di utilizzo, VRAM e temperatura NVIDIA T1000 8GB su Debian confermate dall'utente il 4 ottobre 2026; potenza non supportata gestita. Backend Intel implementato e verificato con fixture `i915`/`xe`: [verifica Intel](docs/INTEL-VERIFICATION.md). Report Intel reale su HP ZBook del 6 ottobre 2026: discovery `i915` e temperature CPU confermate; utilizzo GPU con permessi insufficienti senza privilegi e contatore aggregato indisponibile anche con `sudo`; VRAM locale e temperatura GPU indisponibili in entrambi i report. Macchina identificata: HP ZBook Firefly 16 G11, Core Ultra 7 155H, Arc Meteor Lake-P, Fedora 44 KDE, kernel `7.1.13-200.fc44.x86_64`; loader `1.28.6-1.fc44`, runtime Intel `26.22.38646.6-4.fc44`. Confronto delle metriche, versioni e benchmark GPU restano aperti; la validazione AMD reale è in sospeso su richiesta dell'utente.

## M5 — Processi e interazione stile top/htop

**Dipendenza:** M4.

**Risultato:** tabella processi navigabile, filtrabile e ordinabile.

- [x] Raccogliere PID, nome, CPU e RAM quando la vista li richiede.
- [x] Definire e documentare la percentuale CPU dei processi, incluso il caso oltre il 100%.
- [x] Gestire processi terminati durante la lettura e riutilizzo dei PID.
- [x] Implementare ordinamento, filtro, scrolling e selezione stabile.
- [x] Evitare letture di dettagli costosi per processi non selezionati.
- [x] Verificare il comportamento con molti processi e creare un benchmark dedicato.
- [x] Confermare che nascondere la vista sospenda la relativa raccolta.

**Completamento:** filtro e navigazione restano reattivi, selezione corretta dopo riordinamenti, raccolta assente quando non richiesta. Documentare il consumo a diverse numerosità di processi e fissare un budget basato sulle misure.

**Verifica M5:** [metriche, test e benchmark](docs/M5-VERIFICATION.md). 22 test Rust e regressioni PTY/hardware superati. Dashboard reale: 300 s, CPU 0,277% di un core, RSS finale 4480 KiB, p95 input-render 2,72 ms. Con 10000 record simulati: CPU collector/vista massima 5,329% a 2 s, p95 sequenza filtro 47,86 ms. Zero letture stat mentre la tabella è nascosta, in pausa o con aiuto. M4 conserva la validazione hardware aperta; M5 è stata avviata su richiesta esplicita dell’utente.

## M6 — Rifinitura e prima release Linux

**Dipendenza:** M5.

**Risultato:** release documentata, distribuibile e verificata.

- [ ] Revisionare contrasto, modalità senza colore, ASCII e coerenza delle unità.
- [ ] Verificare terminali diversi, resize ripetuti e uscita con Ctrl-C/errori.
- [ ] Eseguire una sessione prolungata di almeno 8 ore e controllare memoria e stabilità.
- [ ] Profilare e correggere i principali costi CPU; ripetere solo i benchmark interessati dalle modifiche.
- [ ] Pubblicare risultati separati per dashboard base, GPU e processi.
- [ ] Completare test significativi, `cargo fmt --check`, `cargo clippy` e CI.
- [ ] Documentare installazione, scorciatoie, configurazione, metriche e compatibilità hardware.
- [ ] Verificare nome del progetto, licenza e dipendenze prima della pubblicazione.
- [ ] Preparare binario Linux e note di release con limiti conosciuti.

**Completamento:** tutte le categorie previste sono implementate, assenze gestite correttamente, verifiche richieste concluse e risultati di prestazione pubblicati con ambiente e metodo. Ogni limite hardware resta esplicito.

## Dopo la prima release

- [ ] Ampliare la matrice hardware NVIDIA/AMD/Intel e le capacità dei relativi backend.
- [ ] Valutare ulteriori ottimizzazioni in base ai profili reali.
- [ ] Valutare esportazione dei campioni e modalità headless.
- [ ] Valutare altri sistemi operativi mantenendo separati i backend.

## Registro di avanzamento

Per ogni milestone completata aggiungere data, commit o versione, evidenze di verifica, benchmark e limiti residui. Aggiornare le checklist solo quando il risultato è stato verificato.

### M1 — 3 ottobre 2026

Completata nella versione `0.1.0`. Test Rust, Clippy, formattazione e smoke test del terminale superati. Catture salvate in `docs/previews/`; dettagli in [M1-VERIFICATION.md](docs/M1-VERIFICATION.md). Il consumo CPU non è ancora misurato e tutti i dati sono simulati.

### M2 — 3 ottobre 2026

Collector Linux reali per CPU, RAM, load, rete, I/O e spazio filesystem, modalità headless e benchmark. Fixture, confronti con strumenti di sistema e controlli Rust superati. Tre run da 300 s dopo 30 s di riscaldamento ciascuno: CPU media 0,130% di un core, RSS 2944 KiB, zero errori e scadenze saltate. Baseline nel sandbox con rete loopback; confronto live anche su eth0 fuori dal sandbox. TUI reale e relativo budget restano in M3. Evidenze: [M2-VERIFICATION.md](docs/M2-VERIFICATION.md).

### M3 — 4 ottobre 2026

Dashboard live per CPU, RAM, dischi e rete, storico timestampato limitato, worker con ultimo snapshot, pausa e configurazione TOML. Dodici test Rust, smoke test terminale, verifiche CLI, Clippy e formattazione superati. Sessione truecolor 120×40 a 1 Hz: 3600 s misurati, CPU media 0,330% di un core, RSS stabile a 3840 KiB, 3 thread, ripristino terminale verificato. Tre finestre da 300 s entro il budget; latenza p95 5,57 ms con storici pieni. Versione di sviluppo `0.1.0`, hash del binario e condizioni in [M3-VERIFICATION.md](docs/M3-VERIFICATION.md). Misura su PTY drenato, rete loopback nel sandbox; GPU, temperature e processi restano demo per M4/M5.
