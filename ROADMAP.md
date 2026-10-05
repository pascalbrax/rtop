# Roadmap

## Visione

Realizzare un monitor per Linux in Rust e Ratatui, ispirato a top/htop, con una TUI di qualità visiva eccellente e il minor consumo CPU possibile. Nome provvisorio: `rtop`, da verificare prima della pubblicazione.

La qualità visiva si misura attraverso leggibilità, coerenza, navigazione e adattamento al terminale. L'efficienza si misura con benchmark ripetibili fin dalle prime fasi.

## Ambito della prima release

- CPU: utilizzo totale e per core, frequenze disponibili e load average.
- GPU NVIDIA e AMD: utilizzo, memoria e temperatura secondo le capacità del dispositivo; potenza e ventole quando disponibili.
- RAM: totale, disponibile, utilizzata, cache e swap, con definizioni esplicite.
- Dischi: spazio per filesystem e velocità di lettura/scrittura per dispositivo.
- Rete: RX/TX al secondo e contatori per interfaccia.
- Temperature: sensori CPU e GPU identificati e distinti.
- Processi: PID, nome, CPU e RAM, filtro e ordinamento.
- Configurazione TOML, argomenti CLI, aiuto e comando diagnostico `doctor`.

Il backend Intel GPU opzionale è incluso in M4 insieme a NVIDIA/AMD. Altri sistemi operativi, controllo dei processi e notifiche sono sviluppi successivi. La disponibilità di ogni metrica dipende da hardware, driver e permessi.

## Direzione visiva

- Dashboard con CPU, GPU e RAM in evidenza; rete, dischi e temperature facilmente confrontabili.
- Palette contenuta, temi chiaro/scuro, contrasto leggibile e colori semantici coerenti.
- Grafici con unità, scale e finestre temporali esplicite.
- Allineamenti stabili: i valori che cambiano non devono spostare il layout.
- Layout ampio, compatto e a pannello singolo; fallback ASCII e modalità senza colore.
- Navigazione da tastiera e dettagli accessibili selezionando un pannello.
- Nessuna animazione continua o decorazione che richieda risvegli periodici.

## Architettura proposta

Un singolo crate iniziale con moduli separati per CLI, configurazione, modello, storico, collector e UI.

I collector producono snapshot con timestamp monotono, unità e stato della metrica. Il modello distingue valore valido, raccolta iniziale, dato obsoleto, metrica non supportata ed errore. Un coordinatore gestisce le scadenze; la UI riceve gli ultimi dati tramite un canale limitato e resta reattiva durante la raccolta.

Usare Rust, Ratatui e Crossterm. Preferire letture mirate da `/proc` e `/sys`, NVML per NVIDIA e interfacce del driver per AMD. Valutare `sysinfo` per singole esigenze mediante benchmark. Partire con thread standard e canali, introducendo runtime asincroni solo per esigenze dimostrate.

## Budget di prestazioni

Obiettivo preliminare della dashboard a 1 Hz: **consumo medio ≤0,5% di un singolo core** sulla macchina di riferimento. È un obiettivo da validare, non una garanzia universale. Misurare separatamente dashboard, processi e backend GPU.

Definire la macchina di riferimento e il protocollo nella milestone M2. Registrare CPU totale del processo, tempo dei collector, risvegli al secondo, memoria residente e latenza dell'input. Il consumo CPU è `tempo CPU / tempo trascorso × 100`, senza normalizzazione sul numero di core.

Strategie obbligatorie:

- Rendering solo su input, resize o modifiche visibili dei dati; nessun ciclo continuo.
- Riutilizzo dei buffer e storico limitato; niente code di snapshot accumulate.
- Nessun subprocess periodico per raccogliere metriche.
- Scansione dei processi solo quando richiesta dalla vista.
- Frequenze configurabili per gruppo di metriche.
- Raccolta sospesa durante la pausa, salvo opzione esplicita per mantenerla.
- Gestione dei backend lenti senza bloccare la UI o generare worker illimitati.

| Gruppo | Intervallo iniziale |
| --- | --- |
| CPU, RAM, rete, I/O disco | 1 s |
| Utilizzo GPU | 1 s |
| Temperature | 2 s |
| Processi visibili | 2 s |
| Spazio filesystem | 30 s |
| Discovery dispositivi | Avvio e intervallo lungo configurabile |

Storico iniziale: 120 campioni per serie. Grafici con frequenze diverse devono rispettare i timestamp, senza rappresentare campioni distanti come equidistanti nel tempo.

Per la tabella processi, M5 aggiunge un worker attivato dalla visibilità, una lettura di `stat` per PID e ordinamento/filtro memorizzati fra i redraw. Definizioni, misure per numerosità e budget: [M5-VERIFICATION.md](docs/M5-VERIFICATION.md).

## Sequenza di sviluppo

1. **M1 — Design e prototipo:** validare aspetto e navigazione con dati simulati.
2. **M2 — Raccolta e benchmark:** dimostrare correttezza e costo delle metriche di base.
3. **M3 — Dashboard reale:** integrare dati, storico, configurazione e layout adattivo.
4. **M4 — GPU e sensori:** validare NVIDIA, AMD e temperature su hardware reale.
5. **M5 — Processi:** completare l'esperienza top/htop mantenendo il budget misurato.
6. **M6 — Rifinitura e release:** consolidare accessibilità, prestazioni, distribuzione e documentazione.

I criteri di completamento sono in [MILESTONES.md](MILESTONES.md). Ogni fase richiede evidenze prima di passare alla successiva; le date verranno stimate dopo M2.

## Correttezza e casi limite

- Calcolare velocità e utilizzo da differenze dei contatori e tempo monotono realmente trascorso.
- Trattare primo campione, reset dei contatori e rimozione dei dispositivi senza picchi artificiali.
- Distinguere filesystem, partizioni e dispositivi per evitare doppi conteggi.
- Non sommare interfacce fisiche e virtuali indiscriminatamente.
- Identificare sensori per dispositivo ed etichetta, senza fissare indici `hwmon`.
- Mantenere separati CPU package/core e GPU edge/hotspot; mostrare `N/D` quando manca una metrica.
- Segnalare dati obsoleti e permessi mancanti senza interrompere il monitor.
- Ripristinare il terminale su uscita normale, errori e panic gestibili.

## Riferimenti tecnici

- [Ratatui: terminale e ciclo applicativo](https://docs.rs/ratatui/latest/ratatui/struct.Terminal.html)
- [sysinfo](https://docs.rs/sysinfo/latest/sysinfo/)
- [NVIDIA NVML](https://developer.nvidia.com/management-library-nvml)
- [Linux AMDGPU: monitoraggio termico](https://www.kernel.org/doc/html/next/gpu/amdgpu/thermal.html)
- [Linux thermal sysfs](https://cdn.kernel.org/doc/html/latest/driver-api/thermal/sysfs-api.html)
