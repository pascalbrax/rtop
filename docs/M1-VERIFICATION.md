# M1 — Evidenze di completamento

Data: 3 ottobre 2026. Versione: `rtop 0.1.0`, prototipo con dati simulati.

## Risultato

Progetto Rust avviato con Ratatui 0.29, Crossterm 0.28 e Clap 4.6; dipendenze fissate in `Cargo.lock`. Dashboard con sei pannelli, selezione e viste dedicate, temi chiaro/scuro, ASCII, modalità senza colore, aiuto e pausa.

A 80×24 è visibile una sezione alla volta. A 120×40 e 160×50 sono visibili tutti i pannelli. Il limite minimo è 60×18, con messaggio sotto la soglia. Dati mancanti e nomi lunghi sono inclusi nelle fixture.

## Verifiche eseguite

- `cargo test --offline`: 3 test superati. La matrice di rendering copre 4 dimensioni × 4 modalità visive × 7 sezioni, oltre a navigazione, aiuto e terminale piccolo.
- `cargo clippy --offline --all-targets -- -D warnings`: superato.
- `cargo fmt --check`: superato.
- `cargo build --offline`: superato.
- `python3 tests/terminal_smoke.py`: superato su pseudo-terminale Linux; verifica rendering, pausa senza output periodico, resize a quattro dimensioni, navigazione, uscita q/Ctrl-C, uscita dall'alternate screen e ripristino degli attributi termios.
- Catture testuali salvate per 80×24, 120×40 e 160×50; catture SVG del buffer per tema chiaro e scuro, più anteprima ASCII senza colore.

Il rendering è stato controllato attraverso le catture testuali e i test del buffer. Le catture SVG preservano colori e glifi; non attestano la resa su ogni emulatore di terminale.

## Limiti e prossima fase

Non ci sono collector reali, configurazione TOML, raccolta reale dei processi o benchmark CPU: appartengono alle milestone successive. Le tracce storiche sono fixture derivate da una sequenza condivisa, con profili diversi per pannello, per validare la composizione visiva. I grafici non rappresentano ancora serie temporali delle singole metriche.

Il ripristino dopo uscita normale e Ctrl-C è verificato automaticamente. La gestione di panic ed errori si basa sul panic hook Ratatui e sulla guardia di ripristino; non è stata verificata attraverso fault injection. Arresti non gestibili, come SIGKILL, non consentono cleanup.

L'obiettivo ≤0,5% di un core resta da misurare in M2. La preferenza estetica finale richiederà feedback sulla dashboard mentre il progetto evolve.

## Aggiornamento del tema dal riferimento grafico

Adottati sfondo antracite, superfici scure, palette luminosa per sezione, barre segmentate e grafici a linee con griglia tenue. CPU/GPU occupano la fascia superiore, RAM tutta la larghezza e gli altri tre pannelli la fascia inferiore. Tema chiaro, ASCII e navigazione preservati. Ripetuti test di rendering, Clippy e smoke test del terminale; catture aggiornate. Nessun benchmark CPU eseguito in questa fase.

## Pannello processi nella fascia centrale

RAM occupa metà della fascia centrale, affiancata da una tabella di processi simulati ordinati per CPU. Colonne PID, nome, CPU% e RAM; colore giallo dedicato. Il tasto 7 e la navigazione tra sezioni consentono di aprire il pannello anche su terminali compatti. Raccolta reale, filtro e ordinamento interattivo restano in M5. Test Rust e Clippy superati; anteprime rigenerate.
