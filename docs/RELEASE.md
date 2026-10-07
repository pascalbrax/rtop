# Distribuzione Linux — 0.1.0-rc.1

Candidato preparato il 7 ottobre 2026. M6 resta aperta finché la sessione di stabilità di almeno 8 ore e le verifiche richieste non sono concluse. Non si deduce compatibilità GPU completa dalla sola discovery e non viene pubblicato un tag finale durante queste verifiche.

## Nome e licenza

Il nome `rtop` è già utilizzato da altri progetti, fra cui [rapidloop/rtop](https://github.com/rapidloop/rtop) e [narendasan/rtop](https://github.com/narendasan/rtop). Questa distribuzione identifica esplicitamente `pascalbrax/rtop`; il comando e il repository richiesti dall'utente restano `rtop`. `publish = false` impedisce di tentare la pubblicazione con il nome omonimo su crates.io.

Il progetto è MIT; il testo è in `LICENSE`. Il packaging include i testi di licenza e le attribuzioni presenti nei crate risolti per il target host, con un inventario `THIRD-PARTY.json`. Include anche le dipendenze di build; i pacchetti per altre piattaforme esclusi dal grafo host non vengono presentati come incorporati nel binario Linux. Nessun driver NVIDIA/Intel è distribuito nell'archivio: le librerie opzionali provengono dal sistema dell'utente.

## Preparazione dell'archivio

Richiede Linux, Rust/Cargo 1.95.0 (toolchain verificata), Python 3.11+ e `ldd`. La CI usa Ubuntu 24.04; il pacchetto locale riflette il sistema di build registrato in `BUILD-INFO.json`.

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python3 scripts/package_release.py
# Con le dipendenze già nella cache:
python3 scripts/package_release.py --offline
```

Il risultato in `dist/` è `rtop-0.1.0-rc.1-<target>.tar.gz`, con checksum `.sha256`, binario, configurazione di esempio, note e licenze. Il packaging non installa e non pubblica nulla. Per associare un commit alla build impostare `RTOP_SOURCE_REVISION` al relativo SHA. Ordine, metadati tar e intestazione gzip sono deterministici per gli stessi contenuti; non si promette identità del binario compilato su sistemi/toolchain diversi.

Il binario GNU è dinamico: verificare target e dipendenze in `BUILD-INFO.json`. Non è un eseguibile universale per qualunque Linux, non è una build musl e non è stato provato su ARM. Se la libc di destinazione non è compatibile, compilare sul proprio sistema con `cargo build --release --locked`.

## Installazione dell'archivio

```bash
# Eseguire nella cartella che contiene archivio e checksum:
sha256sum -c rtop-0.1.0-rc.1-<target>.tar.gz.sha256
tar -xzf rtop-0.1.0-rc.1-<target>.tar.gz
cd rtop-0.1.0-rc.1-<target>
./rtop --version
./rtop doctor
./rtop
```

Sostituire `<target>` con il target effettivo, ad esempio `x86_64-unknown-linux-gnu`. Per un'installazione personale:

```bash
mkdir -p "$HOME/.local/bin"
install -m 755 rtop "$HOME/.local/bin/rtop"
```

Conservare la cartella dell'archivio per note e licenze. Non sovrascrivere un altro programma omonimo senza verificarne il percorso (`command -v rtop`). Il programma non richiede root per avviarsi; le singole metriche hardware possono richiedere permessi o risultare indisponibili.

## Verifiche e limiti

La CI esegue formattazione, Rust test, Clippy, build, test PTY/configurazione/processi e fixture GPU, una regressione breve di stabilità e packaging. Carica l'archivio come artifact della build, senza creare una GitHub Release. Il run breve CI non sostituisce la prova di 8 ore.

```bash
cargo build --release --locked
python3 tests/terminal_matrix.py
python3 tests/tui_soak.py --seconds 28800 --warmup 30 --output docs/benchmarks/m6-tui-soak
```

Il test usa una PTY drenata 120×40, collector base a 1 Hz e hardware/processi a 2 s, con una sola istanza dell'app. Il budget locale è ≤0,5% di un core, range RSS finale ≤256 KiB, range RSS completo ≤1 MiB e massimo 5 thread. I report distinguono `running`, `passed` e `failed`; un report JSON da solo non significa che gli assert siano passati. Il costo del terminale grafico/SSH e delle GPU supportate assenti nell'ambiente locale resta escluso.

I profili `TERM` testati in PTY non sostituiscono prove visuali nei terminali reali. L’utente ha riferito una prova d’uso su Gentoo via SSH con Windows Terminal; versioni e singoli controlli visuali non sono specificati. `--ascii --no-color` è il fallback esplicito per terminali semplici; non viene promessa un'autodetection delle capacità.

Stato e risultati: [M6-VERIFICATION.md](M6-VERIFICATION.md). Compatibilità Intel osservata: [INTEL-VERIFICATION.md](INTEL-VERIFICATION.md). AMD reale rimane sospesa su richiesta dell'utente; NVIDIA e Intel conservano la validazione M4 aperta.
