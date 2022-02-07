## Universal nomenclature

room-id = Kod pokoju

## Lobby

lobby-welcome = Witaj! Utwórz pokój lub dołącz do istniejącego aby rozpocząć.

lobby-nickname =
   .label = Nazwa
   .hint = Nazwa widziana przez innych
lobby-relay-server =
   .label = Serwer Relay
   .hint = URL serwera

lobby-join-a-room =
   .title = Dołącz do pokoju
   .description =
      Spytaj partnera o kod pokoju
      i wpisz go poniżej.
lobby-room-id =
   .label = { room-id }
   .hint = 6 znaków
lobby-join = Dołącz

lobby-host-a-new-room =
   .title = Utwórz nowy pokój
   .description =
      Utwórz czystą kartkę lub załaduj ją z pliku
      i podziel się kodem pokoju ze znajomymi.
lobby-host = Utwórz
lobby-host-from-file = z pliku

switch-to-dark-mode = Przełącz na tryb ciemny
switch-to-light-mode = Przełącz na tryb jasny
open-source-licenses = Licencje open source

fd-supported-image-files = Obsługiwane formaty obrazów
fd-png-file = Obrazek PNG
fd-netcanv-canvas = Kartka NetCanv

connecting = Łączenie…

## Paint

paint-welcome-host =
   Witaj w swoim pokoju!
   Aby zaprosić znajomych, wyślij im kod pokoju z menu w prawym dolnym rogu ekranu.

unknown-host = <nieznany>
you-are-the-host = Jesteś gospodarzem
someone-is-your-host = jest twoim gospodarzem
room-id-copied = Kod pokoju skopiowany do schowka

someone-joined-the-room = { $nickname } dołączył do pokoju
someone-left-the-room = { $nickname } opuścił pokój
someone-is-now-hosting-the-room = { $nickname } został gospodarzem pokoju
you-are-now-hosting-the-room = Zostałeś gospodarzem pokoju

tool-selection = Zaznaczenie
tool-brush = Pędzel
tool-eyedropper = Pipeta

brush-thickness = Grubość

action-save-to-file = Zapisz do pliku

## Color picker

click-to-edit-color = Kliknij aby edytować kolor
eraser = Gumka
rgb-hex-code = Kod koloru RGB

## Errors

error = Błąd: { $error }
error-fatal = Błąd: { $error }

error-io = I/O: { $error }
error-failed-to-persist-temporary-file = Nie udało się zachować pliku tymczasowego: { $error }
error-image = Błąd operacji na obrazach: { $error }
error-join = Nie udało się złączyć z wątkiem: { $error }
error-channel-send = Kanał do komunikacji z wątkiem został zamknięty
error-toml-parse = Błąd odczytywania TOML: { $error }
error-toml-serialization = Błąd serializacji TOML: { $error }
error-invalid-utf8 = Tekst zawiera niepoprawną sekwencję UTF-8

error-number-is-empty = Liczba nie może być pusta
error-invalid-digit = Liczba zawiera niepoprawną liczbą
error-number-too-big = Liczba jest zbyt duża (przekroczyła zakres liczb całkowitych)
error-number-too-small = Liczba jest zbyt mała (przekroczyła zakres liczb całkowitych)
error-number-must-not-be-zero = Liczba nie może być zerem
error-invalid-number = Niepoprawna liczba (prosimy zgłosić to jako bug)

error-could-not-initialize-backend = Nie udało się zinicjalizować renderera: { $error }
error-could-not-initialize-logger = Nie udało się zinicjalizować loggera: { $error }
error-could-not-initialize-clipboard = Nie udało się zinicjalizować schowka: { $error }

error-config-is-already-loaded = Konfiguracja użytkownika została wcześniej załadowana. Prosimy to zgłośić

error-clipboard-was-not-initialized = Schowek nie został zinicjalizowany. Spróbuj uruchomić ponownie aplikację oraz zgłosić problem jeśli to nie pomoże
error-cannot-save-to-clipboard = Nie udało się zapisać od schowka: { $error }
error-clipboard-does-not-contain-text = Schowek nie zawiera tekstu
error-clipboard-does-not-contain-an-image = Schowek nie zawiera obrazka
error-clipboard-content-unavailable = Zawartość schowka nie jest dostępna w odpowiednim formacie. Spróbuj ponownie skopiować to, co próbujesz wkleić
error-clipboard-not-supported = Schowek nie jest obsługiwany na twoim systemie
error-clipboard-occupied = Schowek jest zajęty. Spróbuj jeszcze raz
error-clipboard-conversion = Nie można skonwertować danych do/z formatu specyficznego dla schowka. Spróbuj ponownie lub zgłoś błąd
error-clipboard-unknown = Nieznany błąd schowka: { $error }

error-translations-do-not-exist = Tłumaczenia dla języka { $language } nie są dostępne
error-could-not-load-language = Nie udało się załadować języka { $language }. Sprawdź konsolę dla szczegółów

error-could-not-open-web-browser = Nie udało się otworzyć przeglądarki
error-no-licensing-information-available =
   NetCanv został skompilowany bez cargo-about. Informacje o licencjach nie są dostępne

error-non-rgba-chunk-image = Otrzymano obraz chunka w formacie innym niż RGBA
error-invalid-chunk-image-format = Niepoprawny format obrazu chunka (ani PNG, ani WebP)
error-invalid-chunk-image-size = Otrzymano obraz chunka o niepoprawnym rozmiarze
error-nothing-to-save = Nie ma nic do zapisu! Narysuj coś na kartce i spróbuj ponownie.
error-invalid-canvas-folder = Wybierz poprawny folder z kartką (o końcówce .netcanv)
error-unsupported-save-format = Nieobsługiwany format zapisu. Wybierz .png lub .netcanv
error-missing-canvas-save-extension = Nie można zapisać kartki bez rozszerzenia pliku. Wybierz .png lub .netcanv
error-invalid-chunk-position-pattern = Pozycja chunka powinna spełniać wzór: x,y
error-trailing-chunk-coordinates-in-filename = Dodatkowe współrzędne znalezione po pozycji x,y
error-canvas-toml-version-mismatch = Niezgodność wersji w canvas.toml. Spróbuj pobrać nowszego NetCanva

error-dialog-unexpected-output = Nieoczekiwany błąd przy otwieraniu dialogu: { $output }
error-no-dialog-implementation = Dialogi nie są obsługiwane na twoim systemie
error-dialog-implementation-error = Błąd implementacji dialogu: { $error }

error-invalid-url = Niepoprawny URL. Sprawdź czy nie posiada błędów w pisowni
error-no-version-packet = Nie otrzymano pakietu wersji od serwera
error-invalid-version-packet = Serwer wysłał niepoprawny pakiet wersji
error-relay-is-too-old = Wersja Relaya jest przestarzała. Spróbuj połączyć się z innym serwerem lub pobrać starego NetCanva
error-relay-is-too-new = Wersja Relaya jest zbyt nowa. Spróbuj pobrać nowszego NetCanva
error-received-packet-that-is-too-big = Otrzymano pakiet, który był zbyt duży
error-tried-to-send-packet-that-is-too-big = Nie można wysłać pakietu większego niż { $max } bajtów (próbowano wysłać { $size })
error-tried-to-send-packet-that-is-way-too-big = Nie można wysłać pakietu o rozmiarze większym niż limit 32-bitowych liczb całkowitych
error-relay-has-disconnected = Serwer Relay rozłączył się
error-web-socket = Błąd komunikacji WebSocket: { $error }

error-not-connected-to-relay = Nie można wysłać pakietu: brak połączenia z serwerem
error-not-connected-to-host = Nie można wysłać pakietu: brak połączenia z gospodarzem
error-packet-serialization-failed = Niepoprawny pakiet: { $error }
error-packet-deserialization-failed = Niepoprawny pakiet: { $error }
error-relay =
   .no-free-rooms = Nie udało się znaleźć wolnego pokoju. Spróbuj ponownie
   .no-free-peer-ids = Serwer jest pełny. Spróbuj połączyć się z innym serwerem
   .room-does-not-exist = Pokój o podanym kodzie nie istnieje. Sprawdź czy kod nie zawiera literówek
   .no-such-peer = Błąd wewnętrzny serwera: Nie ma takiej osoby
error-unexpected-relay-packet = Serwer wysłał niepoprawny pakiet; prawdopodobnie został zmodyfikowany i jest potencjalnie niebezpieczny
error-client-is-too-old = Wersja NetCanv jest zbyt stara. Pobierz nowszą wersję aby dołączyć do tego pokoju
error-client-is-too-new = Wersja NetCanv jest zbyt nowa. Dołącz do innego pokoju lub pobierz starszą wersję

error-invalid-tool-packet = Otrzymano niepoprawny pakiet narzędzia

error-nickname-must-not-be-empty = Nazwa nie może być pusta
error-nickname-too-long = Maksymalna długość nazwy to { $max-length } znaków
error-invalid-room-id-length = { room-id } musi być kodem o { $length } znakach
error-while-performing-action = Błąd podczas wykonywania akcji: { $error }
error-while-processing-action = Błąd podczas przetwarzania akcji: { $error }
