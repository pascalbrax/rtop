# Grafici ad area

Scelta approvata dall'utente e implementata il 5 ottobre 2026: area scura con contorno luminoso, in sostituzione delle tracce a punti Braille. I colori restano quelli del rispettivo pannello. Il renderer è condiviso fra dashboard reale e anteprima demo.

Il contorno usa blocchi pieni e mezzi blocchi Unicode (`█▀▄`): due righe di pixel per cella, senza animazioni o nuovi timer. Il riempimento mescola il colore del pannello con la superficie al 24%; il contorno mantiene il colore luminoso originale. La griglia è un solo riferimento orizzontale tenue. Tema chiaro e scuro usano le palette già presenti.

Disco e rete mostrano l'area della prima serie (lettura/RX) e il contorno chiaro della seconda (scrittura/TX). Non vengono sommate. La modalità senza colore mostra solo i contorni; quella ASCII mantiene lo storico compatto originale.

L'asse temporale conserva la finestra e le unità dei collector. Si interpolano solo campioni appartenenti allo stesso segmento: errori, reset, pause e intervalli mancanti lasciano vuoti nell'area. Nessun riempimento viene esteso oltre il primo o l'ultimo campione di un segmento. Un campione isolato occupa una sola colonna. Quando più campioni ricadono nella stessa colonna, viene mantenuto il picco più alto. Le coordinate vengono limitate all'area visibile e i valori fuori scala al massimo/minimo della scala.

Lavoro e memoria del raster dipendono dalla dimensione del terminale e dallo storico limitato; non dal tempo totale di esecuzione. Il renderer scrive direttamente nel buffer Ratatui. L'esportazione SVG rappresenta i blocchi come rettangoli, per conservare riempimenti e mezze celle senza dipendere dalla geometria dei glifi nel visualizzatore SVG.

## Verifiche

- 20 test Rust superati, inclusi campioni isolati, compressione dei picchi, lacune, seconda serie, modalità senza colore, valori non finiti e aree piccole/vuote.
- Test esistenti di layout e temi, errori/staleness hardware e assenza di GPU superati.
- Formattazione e Clippy con warning come errori superati.
- Smoke test su PTY: pausa senza redraw periodico, resize, input, uscita e ripristino del terminale superati.
- Configurazione CLI/TOML e precedenza `NO_COLOR` verificate.

Anteprima con dati **simulati**, non una misura hardware: [SVG 120×40](previews/area-dark-120x40.svg).

Comandi di misura su dashboard reale:

```sh
cargo build --release --offline
python3 tests/tui_soak.py --seconds 300 --warmup 30 --output docs/benchmarks/area-tui-soak
python3 tests/tui_latency.py --output docs/benchmarks/area-input-latency.json --warmup 125
```

Le misure locali usano un PTY 120×40 drenato, truecolor, collector base a 1 Hz e hardware a 2 s. Escludono il costo dell'emulatore grafico e delle GPU supportate reali. La latenza è misurata su un processo separato; non è un confronto controllato con la build precedente. I risultati storici M3/M4/Intel restano invariati nei rispettivi report. La verifica Intel su Fedora resta aperta e il test AMD resta sospeso.

## Risultati locali

Build release SHA-256 `557dfd7d51190c988d7bd4ef57a8e51f1153476290086b2c3faf8b2240ffa841`.

| Misura | Risultato |
| --- | --- |
| Durata misurata, dopo 30 s di warmup | 300 s |
| CPU media di un core | 0,257% |
| RSS iniziale / finale | 4096 / 4224 KiB |
| Variazione RSS nella coda | 128 KiB |
| Latenza p50 / p95 / massima | 1,39 / 2,14 / 2,46 ms |
| Campioni di latenza / warmup storico | 40 / 125 s |
| Ripristino terminale | Verificato |

Dati grezzi: [soak JSON](benchmarks/area-tui-soak.json), [campioni CSV](benchmarks/area-tui-soak.csv), [latenza](benchmarks/area-input-latency.json). La macchina locale ha una GPU Matrox senza backend supportato e sensori CPU disponibili. Queste misure verificano il renderer con collector reali, ma non sostituiscono un benchmark NVIDIA o Intel reale.
