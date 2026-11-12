# Changelog

## 0.0.9 (in progress)

### English

- **The long calls no longer freeze the file window.** Rescan, upload, download (and going on with an unfinished upload) run on a worker of the file workbench (fs-components v0.0.12, after `Session::submit`): the window answers while they run, shows "Running...", a transfer row says "running" until the server has answered, and a second start meanwhile is refused with a note (not queued, so nothing is written twice). What was typed is kept; a failed call gives the input back. There is no abort and no progress: the server's contract has neither.
- **Limit that remains:** lists, the folder tree, rename, move, copy, trash and the preview are still called on the interface thread, each with its time limit (30 s; move, copy, restore and purge 15 min). The server answers them while a transfer runs (the connection serves calls side by side), but a server that does not answer holds the window for that time.
- Built on fs-components v0.0.12.

### Deutsch

- **Die langen Aufrufe frieren das Dateifenster nicht mehr ein.** Rescan, Hochladen, Herunterladen (und das Fortsetzen eines unfertigen Hochladens) laufen auf einem Arbeiter der Dateiansicht (fs-components v0.0.12, nach `Session::submit`): das Fenster antwortet währenddessen, zeigt „Läuft...“, eine Übertragungszeile steht auf „läuft“, bis der Server geantwortet hat, und ein zweiter Start in der Zeit wird mit Hinweis abgelehnt (nicht eingereiht, damit nichts doppelt geschrieben wird). Getipptes bleibt; ein fehlgeschlagener Aufruf gibt die Eingabe zurück. Es gibt keinen Abbruch und keinen Fortschritt: der Vertrag des Servers hat keines von beiden.
- **Grenze, die bleibt:** Listen, der Ordnerbaum, Umbenennen, Verschieben, Kopieren, Papierkorb und die Vorschau werden weiter im Oberflächen-Thread aufgerufen, jeweils mit Zeitgrenze (30 s; Verschieben, Kopieren, Wiederherstellen und Endgültig löschen 15 min). Der Server beantwortet sie, während eine Übertragung läuft (die Verbindung bedient Aufrufe nebeneinander), aber ein Server, der nicht antwortet, hält das Fenster so lange fest.
- Gebaut auf fs-components v0.0.12.

## 0.0.8

### English

- **The file window is the standard window.** `fs-files` opens the file view; the generated Admin frontend is behind `--admin` or `--facet admin`. `--view list|grid` chooses how it starts, `--rescan` takes over what lies on the disk first.
- **Deliberate change of behaviour: folders first.** Every listing (`list` and `find`) now puts the folders before the files, each group in the order asked for. Before, folders and files were mixed by name. The server takes `Sort.foldersFirst` of the contract (fs-interfaces v0.0.19): the value of the client is respected, and a client that does not write it gets folders first. A client that wants the mixed order sends `false`.
- **Types of files.** The media type is read from the first bytes, then from the ending of the name, then by whether the content reads as text; `application/octet-stream` only when nothing fits. Files taken over by a rescan or uploaded without a type get it.
- The window pins the folders of the storage that have the names of the user folders of the person (`user-dirs.dirs`).
- Built on fs-components v0.0.11 and fs-interfaces v0.0.19 (Blinc fs-blinc-3.1).
- For the typing test only: the optional feature `test-access` (debug builds; fs-components refuses it in a release build) lets the test read what is typed into the fields.

### Deutsch

- **Das Dateifenster ist das Standardfenster.** `fs-files` öffnet die Dateiansicht; das erzeugte Admin-Frontend gibt es mit `--admin` oder `--facet admin`. `--view list|grid` wählt die Startansicht, `--rescan` übernimmt vorher, was auf der Platte liegt.
- **Bewusste Verhaltensänderung: Ordner zuerst.** Jede Auflistung (`list` und `find`) zeigt jetzt die Ordner vor den Dateien, jede Gruppe in der gewünschten Reihenfolge. Vorher waren Ordner und Dateien nach Namen gemischt. Der Server übernimmt `Sort.foldersFirst` des Vertrags (fs-interfaces v0.0.19): der Wert des Clients gilt, ein Client, der ihn nicht schreibt, bekommt Ordner zuerst. Wer die gemischte Reihenfolge will, sendet `false`.
- **Dateitypen.** Der Medientyp wird aus den ersten Bytes gelesen, dann aus der Endung des Namens, dann danach, ob der Inhalt als Text lesbar ist; `application/octet-stream` nur, wenn nichts passt. Das gilt für Dateien, die ein Rescan übernimmt oder die ohne Typ hochgeladen werden.
- Das Fenster heftet die Ordner der Ablage an, die so heißen wie die Benutzerordner der Person (`user-dirs.dirs`).
- Gebaut auf fs-components v0.0.11 und fs-interfaces v0.0.19 (Blinc fs-blinc-3.1).
- Nur für den Tipptest: das optionale Feature `test-access` (Debug-Builds; fs-components verweigert es in einem Release-Build) lässt den Test lesen, was in die Felder getippt wird.
