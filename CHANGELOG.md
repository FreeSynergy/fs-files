# Changelog

## 0.0.8 (in progress)

### English

- **The file window is the standard window.** `fs-files` opens the file view; the generated Admin frontend is behind `--admin` or `--facet admin`. `--view list|grid` chooses how it starts, `--rescan` takes over what lies on the disk first.
- **Deliberate change of behaviour: folders first.** Every listing (`list` and `find`) now puts the folders before the files, each group in the order asked for. Before, folders and files were mixed by name. The server takes `Sort.foldersFirst` of the contract (fs-interfaces v0.0.19): the value of the client is respected, and a client that does not write it gets folders first. A client that wants the mixed order sends `false`.
- **Types of files.** The media type is read from the first bytes, then from the ending of the name, then by whether the content reads as text; `application/octet-stream` only when nothing fits. Files taken over by a rescan or uploaded without a type get it.
- The window pins the folders of the storage that have the names of the user folders of the person (`user-dirs.dirs`).
- Built on fs-components v0.0.11 and fs-interfaces v0.0.19 (Blinc fs-blinc-3.1).
- **Known limit: the window calls the server on the interface thread.** A long operation (upload, download, copy, move, restore, purge, rescan; limit 15 min, other calls 30 s) freezes the window until it ends. The non-blocking path is the next round (0.0.9).
- For the typing test only: the optional feature `test-access` (debug builds; fs-components refuses it in a release build) lets the test read what is typed into the fields.

### Deutsch

- **Das Dateifenster ist das Standardfenster.** `fs-files` öffnet die Dateiansicht; das erzeugte Admin-Frontend gibt es mit `--admin` oder `--facet admin`. `--view list|grid` wählt die Startansicht, `--rescan` übernimmt vorher, was auf der Platte liegt.
- **Bewusste Verhaltensänderung: Ordner zuerst.** Jede Auflistung (`list` und `find`) zeigt jetzt die Ordner vor den Dateien, jede Gruppe in der gewünschten Reihenfolge. Vorher waren Ordner und Dateien nach Namen gemischt. Der Server übernimmt `Sort.foldersFirst` des Vertrags (fs-interfaces v0.0.19): der Wert des Clients gilt, ein Client, der ihn nicht schreibt, bekommt Ordner zuerst. Wer die gemischte Reihenfolge will, sendet `false`.
- **Dateitypen.** Der Medientyp wird aus den ersten Bytes gelesen, dann aus der Endung des Namens, dann danach, ob der Inhalt als Text lesbar ist; `application/octet-stream` nur, wenn nichts passt. Das gilt für Dateien, die ein Rescan übernimmt oder die ohne Typ hochgeladen werden.
- Das Fenster heftet die Ordner der Ablage an, die so heißen wie die Benutzerordner der Person (`user-dirs.dirs`).
- Gebaut auf fs-components v0.0.11 und fs-interfaces v0.0.19 (Blinc fs-blinc-3.1).
- **Bekannte Grenze: das Fenster ruft den Server im Oberflächen-Thread auf.** Eine lange Operation (Hochladen, Herunterladen, Kopieren, Verschieben, Wiederherstellen, Endgültig löschen, Rescan; Grenze 15 min, andere Aufrufe 30 s) friert das Fenster ein, bis sie endet. Der nicht-blockierende Weg ist die nächste Runde (0.0.9).
- Nur für den Tipptest: das optionale Feature `test-access` (Debug-Builds; fs-components verweigert es in einem Release-Build) lässt den Test lesen, was in die Felder getippt wird.
