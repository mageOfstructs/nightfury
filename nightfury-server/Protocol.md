# Protocol

Server communication is done over the UNIX socket "nightfury.sock", which is located in `$XDG_RUNTIME_DIR` or `/run/user/$EUID` if the aforementioned variable isn't set

## Protocol Structure

- all messages except ones consisting of a singular control code must be NUL-terminated
- a NUL-terminated string not starting with a control code will advance the internal FSM-Cursor using said string
  - The Server will respond with the completion as a NUL-terminated string.
  - Should there be no completion possible yet, a single NUL character is returned
- Unicode strings must be UTF-8 encoded

### Control Codes

- 0x01-0x07 (inclusive) are reserved
- `<CC>`: control code

- 0x01: "get capabilities"
  - asks the server which languages are currently supported
  - the response will be of the following format: `lang1;lang2;lang3;...\0`
- 0x02: "install language"
  - TODO: ability to automatically get a language from some central registry
  - proposed format: `<CC><lang>[;<registry_url]\0`
- 0x03: revert
  - causes the `revert()` function to be called on the cursor
- 0x04: reset
  - causes the current cursor to be set back to the fsm root and all internal state be cleared
- 0x05: initialize
  - sets up a new cursor at the root of the specified language fsm
  - format: `<CC><lang>\0`
  - response: 16-bit unsigned integer (cursor handle)
- 0x06: set cursor
  - format: `<CC><cursor_handle>[request]\0`
  - sets the current cursor to `cursor_handle`
  - if request is given, only use the specified cursor for that request, do not update the current cursor state

### Server responses

Aside from request-specific responses, there are some general responses the server can give:

- 0x0: "Ok"; the server processed the request successfully and has nothing to say back
- 0x1: Generic error
  - format: `<CC>[error_message]\0`
