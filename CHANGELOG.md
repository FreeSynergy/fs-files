# Changelog

## 0.0.10 (in progress)

### English

- **The face of the file window is chosen in the window and kept.** The new menus are Program (Quit) and View; View has the entries `Face: List` and `Face: Grid`, the one that is shown says `(current)` and cannot be chosen again. The choice is shown at once and kept in `files-window.conf` (`view=list|grid`) in the data folder (`$XDG_DATA_HOME/freesynergy`, else `~/.local/share/freesynergy`, on Windows the local application data folder); written to a part file and renamed over the old one, so a crash leaves the old or the new file. A missing or damaged file is the standard (the list), never an error; a place that cannot be written is said on stderr and the face still changes for this run. `--view list|grid` stays the override for one start (it is not written).
- **Honest limit: there is one face.** The file window has one face with two ways to show a folder (list, grid; `FileView` of fs-components has no other). A second face (for example a compact or a tile face with big previews) does not exist; the menu offers only what exists. Not in this version: filled MIME and folder icons and preview pictures (planned for 0.0.11), dragging with the mouse, a switch for the side bar.
- The side bar of 0.0.9 holds: pinned folders by the names of the XDG user folders (coloured), Network says `Nothing connected yet` (nothing is built there), no empty strip under the status bar.
- Built on fs-components v0.0.13 (one source, `c97eca0`).

### Deutsch

- **Das Gesicht des Dateifensters wird im Fenster gewählt und gemerkt.** Neu sind die Menüs Program (Quit) und View; View hat die Einträge `Face: List` und `Face: Grid` (die Oberfläche spricht noch Englisch), der gezeigte sagt `(current)` und lässt sich nicht erneut wählen. Die Wahl wirkt sofort und wird in `files-window.conf` (`view=list|grid`) im Datenordner gemerkt (`$XDG_DATA_HOME/freesynergy`, sonst `~/.local/share/freesynergy`, unter Windows der lokale Anwendungsdatenordner); in eine Teildatei geschrieben und über die alte umbenannt, ein Absturz hinterlässt also die alte oder die neue Datei. Eine fehlende oder kaputte Datei ist der Standard (die Liste), nie ein Fehler; ein Ort, der sich nicht beschreiben lässt, wird auf stderr gesagt, und das Gesicht ändert sich für diesen Lauf trotzdem. `--view list|grid` bleibt die Überschreibung für einen Start (wird nicht geschrieben).
- **Ehrliche Grenze: es gibt ein Gesicht.** Das Dateifenster hat ein Gesicht mit zwei Arten, einen Ordner zu zeigen (Liste, Raster; `FileView` von fs-components kennt keine weitere). Ein zweites Gesicht (etwa ein kompaktes oder eine Kachelansicht mit großen Vorschauen) gibt es nicht; das Menü bietet nur, was es gibt. Nicht in dieser Version: gefüllte MIME- und Ordner-Symbole und Vorschaubilder (geplant für 0.0.11), Ziehen mit der Maus, ein Schalter für die Seitenleiste.
- Die Seitenleiste aus 0.0.9 gilt: angeheftete Ordner nach den Namen der XDG-Benutzerordner (farbig), Network sagt `Nothing connected yet` (dort ist nichts gebaut), kein leerer Streifen unter der Statusleiste.
- Gebaut auf fs-components v0.0.13 (eine Quelle, `c97eca0`).

## 0.0.9

### English

- **The long calls no longer freeze the file window.** Rescan, upload, download (and going on with an unfinished upload) run on a worker of the file workbench (fs-components v0.0.12, after `Session::submit`): the window answers while they run, shows "Running...", a transfer row says "running" until the server has answered, and a second start meanwhile is refused with a note (not queued, so nothing is written twice). What was typed is kept; a failed call gives the input back. There is no abort and no progress: the server's contract has neither.
- **Limit that remains:** lists, the folder tree, rename, move, copy, trash and the preview are still called on the interface thread, each with its time limit (30 s; move, copy, restore and purge 15 min). The server answers them while a transfer runs (the connection serves calls side by side), but a server that does not answer holds the window for that time.
- Built on fs-components v0.0.13 (lift; it keeps typed input when a call starts).

### Deutsch

- **Die langen Aufrufe frieren das Dateifenster nicht mehr ein.** Rescan, Hochladen, Herunterladen (und das Fortsetzen eines unfertigen Hochladens) laufen auf einem Arbeiter der Dateiansicht (fs-components v0.0.12, nach `Session::submit`): das Fenster antwortet währenddessen, zeigt „Läuft...“, eine Übertragungszeile steht auf „läuft“, bis der Server geantwortet hat, und ein zweiter Start in der Zeit wird mit Hinweis abgelehnt (nicht eingereiht, damit nichts doppelt geschrieben wird). Getipptes bleibt; ein fehlgeschlagener Aufruf gibt die Eingabe zurück. Es gibt keinen Abbruch und keinen Fortschritt: der Vertrag des Servers hat keines von beiden.
- **Grenze, die bleibt:** Listen, der Ordnerbaum, Umbenennen, Verschieben, Kopieren, Papierkorb und die Vorschau werden weiter im Oberflächen-Thread aufgerufen, jeweils mit Zeitgrenze (30 s; Verschieben, Kopieren, Wiederherstellen und Endgültig löschen 15 min). Der Server beantwortet sie, während eine Übertragung läuft (die Verbindung bedient Aufrufe nebeneinander), aber ein Server, der nicht antwortet, hält das Fenster so lange fest.
- Gebaut auf fs-components v0.0.13 (Anhebung; sie behält Getipptes, wenn ein Aufruf startet).

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
