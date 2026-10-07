# Changelog

## 0.1.0-rc.1 — candidato Linux, 7 ottobre 2026

Prima versione candidata per il repository `pascalbrax/rtop`. La release finale resta subordinata alle verifiche M6; non è stato creato un tag finale.

- Dashboard CPU, RAM, dischi, rete, GPU, temperature e processi reali.
- Palette antracite, un colore distinto per pannello, tema chiaro, ASCII e modalità senza colore; grafici originali a dots Braille.
- Processi raccolti soltanto quando visibili, con filtro, ordinamento e navigazione.
- Worker separati, storico limitato e rendering su eventi; nessun subprocess periodico.
- GPU NVIDIA via NVML, AMD via sysfs, Intel opzionale via Level Zero Sysman; metriche assenti indicate come `N/D`.
- Errori di avvio interattivo senza terminale restituiti senza panic; ripristino anche dopo inizializzazione parziale.
- Licenza MIT, archivio Linux con checksum e licenze delle dipendenze; CI di verifica e packaging.

Limiti hardware: NVIDIA T1000 rilevata su Debian con utilizzo, VRAM e temperatura; potenza non supportata. Intel Arc Meteor Lake-P su Fedora 44/i915 rilevata, ma utilizzo negato senza privilegi e contatore aggregato non trovato con `sudo`; temperatura GPU e memoria locale indisponibili. Validazione AMD reale sospesa. Benchmark e confronti sotto carico GPU restano aperti.
