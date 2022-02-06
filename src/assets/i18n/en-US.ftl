## Universal nomenclature

room-id = Room ID

## Lobby

lobby-welcome = Welcome! Host a room or join an existing one to start painting.

lobby-nickname =
   .label = Nickname
   .hint = Name shown to others
lobby-relay-server =
   .label = Relay server
   .hint = Server URL

lobby-join-a-room =
   .title = Join a room
   .description =
      Ask your friend for the { room-id }
      and enter it into the text field below.
lobby-room-id =
   .label = { room-id }
   .hint = 6 characters
lobby-join = Join

lobby-host-a-new-room =
   .title = Host a new room
   .description =
      Create a blank canvas, or load an existing one from file,
      and share the { room-id } with your friends.
lobby-host = Host
lobby-host-from-file = from File

switch-to-dark-mode = Switch to dark mode
switch-to-light-mode = Switch to light mode
open-source-licenses = Open source licenses

connecting = Connecting…

## Paint

paint-welcome-host =
   Welcome to your room!
   To invite friends, send them the { room-id } from the menu in the bottom right corner of your screen.

unknown-host = <unknown>
you-are-the-host = You are the host
someone-is-your-host = is your host
room-id-copied = { room-id } copied to clipboard

someone-joined-the-room = { $nickname } joined the room
someone-left-the-room = { $nickname } has left
someone-is-now-hosting-the-room = { $nickname } is now hosting the room
you-are-now-hosting-the-room = You are now hosting the room

tool-selection = Selection
tool-brush = Brush
tool-eyedropper = Eyedropper

brush-thickness = Thickness

action-save-to-file = Save to file

## File dialogs

fd-supported-image-files = Supported image files
fd-png-file = PNG file
fd-netcanv-canvas = NetCanv canvas

## Color picker

click-to-edit-color = Click to edit color
eraser = Eraser
rgb-hex-code = RGB hex code

## Errors

failure =
   An error occured: { $message }

   If you think this is a bug, please file an issue on GitHub.
   https://github.com/liquidev/netcanv

error = Error: { $error }
error-fatal = Fatal: { $error }

error-io = I/O: { $error }
error-failed-to-persist-temporary-file = Failed to persist temporary file: { $error }
error-image = Image operation error: { $error }
error-join = Could not join thread: { $error }
error-channel-send = Thread communication channel is closed
error-toml-parse = TOML parse error: { $error }
error-toml-serialization = TOML serialization error: { $error }
error-invalid-utf8 = Invalid UTF-8 found in string

error-number-is-empty = Number must not be empty
error-invalid-digit = Invalid digit found in number
error-number-too-big = Number is too big (and caused integer overflow)
error-number-too-small = Number is too small (and caused integer underflow)
error-number-must-not-be-zero = Number must not be zero
error-invalid-number = Invalid number (please report this)

error-could-not-initialize-backend = Could not initialize backend: { $error }
error-could-not-initialize-logger = Could not initialize logger: { $error }
error-could-not-initialize-clipboard = Could not initialize clipboard: { $error }

error-config-is-already-loaded = User configuration is already loaded. This is a bug, please report this

error-clipboard-was-not-initialized = Clipboard was not initialized properly. Try restarting the app and if the issue persists file a bug
error-cannot-save-to-clipboard = Could not save to clipboard: { $error }
error-clipboard-does-not-contain-text = Clipboard does not contain text
error-clipboard-does-not-contain-an-image = Clipboard does not contain an image
error-clipboard-content-unavailable = Clipboard content is not available in the appropriate format. Try copying the thing you're trying to paste again
error-clipboard-not-supported = Clipboard is not supported on your platform
error-clipboard-occupied = Clipboard is currently occupied. Try again
error-clipboard-conversion = Cannot convert data to/from a clipboard-specific format. Try again or report a bug
error-clipboard-unknown = Unknown clipboard error: { $error }

error-translations-do-not-exist = Translations for { $language } do not exist yet
error-could-not-load-language = Could not load language { $language }. See console log for details

error-could-not-open-web-browser = Could not open web browser
error-no-licensing-information-available =
   NetCanv was built without cargo-about installed. Licensing information is not available

error-non-rgba-chunk-image = Received non-RGBA chunk image
error-invalid-chunk-image-format = Invalid chunk image format (was not PNG nor WebP)
error-invalid-chunk-image-size = Received chunk image of invalid size
error-nothing-to-save = There's nothing to save! Draw something on the canvas and try again
error-invalid-canvas-folder = Please select a valid canvas folder (one whose name ends with .netcanv)
error-unsupported-save-format = Unsupported save format. Choose either .png or .netcanv
error-missing-canvas-save-extension = Can't save canvas without an extension. Choose either .png or .netcanv
error-invalid-chunk-position-pattern = Chunk position must follow the pattern: x,y
error-trailing-chunk-coordinates-in-filename = Trailing coordinates found after x,y
error-canvas-toml-version-mismatch = Version mismatch in canvas.toml. Try downloading a newer version of NetCanv

error-dialog-unexpected-output = Unexpected output while opening dialog: { $output }
error-no-dialog-implementation = Dialogs are not available on your platform
error-dialog-implementation-error = Dialog implementation error: { $error }

error-invalid-url = Could not parse URL. Please double-check if it's correct
error-no-version-packet = Did not receive a version packet from the relay
error-invalid-version-packet = The relay sent an invalid version packet
error-relay-is-too-old = Relay version is too old. Try connecting to a different relay or download an older version of NetCanv
error-relay-is-too-new = Relay version is too new. Try downloading a newer version of NetCanv
error-received-packet-that-is-too-big = Received a packet that exceeds the maximum supported size
error-tried-to-send-packet-that-is-too-big = Cannot send packet that is bigger than { $max } bytes (got { $size })
error-tried-to-send-packet-that-is-way-too-big = Cannot send packet that exceeds the 32-bit integer limit
error-relay-has-disconnected = The relay server has disconnected

error-not-connected-to-relay = Cannot send packet: not connected to relay
error-not-connected-to-host = Cannot send packet: not connected to host
error-packet-serialization-failed = Bad packet: { $error }
error-packet-deserialization-failed = Bad packet: { $error }
error-relay = { $error }
error-unexpected-relay-packet = Bad packet type received from relay; it's probably modified or malicious
error-client-is-too-old = Your version of NetCanv is too old. Try downloading a newer version
error-client-is-too-new = Your version of NetCanv is too new. Join a newer room or download an older version

error-invalid-tool-packet = Invalid tool packet received

error-nickname-must-not-be-empty = Nickname must not be empty
error-nickname-too-long = The maximum length of a nickname is { $max-length } characters
error-invalid-room-id-length = { room-id } must be a code with { $length } characters
error-while-performing-action = Error while performing action: { $error }
error-while-processing-action = Error while processing action: { $error }
