# BrightScript semantics (device-verified)

Behaviors confirmed on the reference device (AOC H302X, Roku OS 16.0.4). Each
entry names the probe in `fidelity/` whose `golden/console.txt` proves it. When
the official documentation disagrees with a golden, the golden wins.

Entries marked **provisional** are implemented from a guess and still need a
dedicated probe.

## Printing

Probes: `language/print-zones`, `language/numbers`.

- A comma advances to the next 16-column tab zone. If the cursor is already
  past the start of a zone, the next one is used (`"1234567890123456", "x"`
  puts `x` at column 32).
- A semicolon concatenates without spacing; a trailing `;` or `,` suppresses
  the newline. `?` is an alias of `print`.
- Numbers print with a leading sign column: a space for non-negative values,
  `-` for negative ones (`print "x"; 1; "y"; -1` gives `x 1y-1`).
- Float and Double also print a **trailing space**; Integer and LongInteger do
  not.
- `PRINT` evaluates every item before writing anything: if a later item raises
  an error, nothing from that statement is printed.

## Number formatting

Probe: `language/numbers`.

- Float formats like C `%g` with 6 significant digits (`123456.7` gives
  `123457`, `1e7` gives `1e+07`, `0.00001` gives `1e-05`).
- Double formats like `%g` with 14 significant digits (`1.0 / 3.0#` gives
  `0.33333333333333`).
- `5.0` prints `5`, never `5.0`.
- **Provisional:** a float literal with 10 or more significant digits is a
  Double (`3.14159265358979` prints `3.1415926535898`). The exact threshold is
  not yet pinned by a probe (`DOUBLE_LITERAL_DIGITS` in `lexer.rs`).

## Numeric operators

Probe: `language/numbers`.

| Expression            | Result                               |
| --------------------- | ------------------------------------ |
| `7 / 2`               | `3.5` (Integer / Integer is Float)   |
| `7 \ 2`, `-7 \ 2`     | `3`, `-3` (truncates toward zero)    |
| `7 mod 3`, `-7 mod 3` | `1`, `-1` (sign of the dividend)     |
| `2 ^ 10`              | `1024`, type Integer                 |
| `2 ^ 0.5`, `10 ^ -2`  | Float                                |
| `2147483647 + 1`      | `2147483648`, promoted (not wrapped) |
| `1 + 1.5`             | Float                                |
| `1.5 + 1.5#`          | Double                               |
| `1& + 1`              | LongInteger                          |
| `3 = 3.0`             | `true`                               |

- Integer `^` with a non-negative integer exponent stays Integer.
- Integer overflow on `+`/`-` produces a wider value instead of wrapping. The
  exact result type (Double vs LongInteger) still needs a probe.
- Compound assignment follows the same rules: `x = 20 : x /= 4` makes `x` a
  Float.
- `int` floors (`int(-3.7)` = `-4`), `fix` truncates (`fix(-3.7)` = `-3`),
  `cint` rounds half away from zero (`cint(2.5)` = `3`).
- Hex literals: `&HFF` and `&hff` are `255`; `-&H10` is `-16`.

## Logical and bitwise operators

Probe: `language/boolean-ops`.

- `and`/`or`/`not` are logical on Booleans and **bitwise** on Integers
  (`12 and 10` = `8`, `not 0` = `-1`).
- `not` binds looser than comparison: `not a = b` is `not (a = b)`.
- `invalid = invalid` is `true`; `<<` and `>>` are bit shifts.

## Strings

Probe: `language/strings`.

- Global functions are **1-based** (`mid(s, 8)`, `instr(1, s, "o")`); `ifString`
  methods are **0-based** (`s.mid(1, 3)`, `s.instr(0, "o")`).
- Comparison is ordinal and case-sensitive (`"a" = "A"` is `false`,
  `"B" > "a"` is `false`).
- `str(5)` is `" 5"`, `stri(42)` is `" 42"` (sign column), `str(-5)` is `"-5"`.
- `val("abc")` is `0`; `val("3 apples")` is `3` (longest numeric prefix).
- `asc("")` is `0`.
- Strings have **no** `ucase`/`lcase` methods: `"abc".ucase()` raises
  `Member function not found` (`&hf4`). Use the global `UCase`/`LCase`.
- `""` inside a literal is an escaped quote.

## Arrays and associative arrays

Probe: `language/collections`.

- AA keys from literals and dot access are lowercased; bracket keys keep their
  case (`aa["D"] = 4` stores `D`).
- Lookup is case-insensitive in every form.
- `Keys()` and `print aa` sort keys case-insensitively.
- Printing an AA or array shows nested containers as `<Component: roArray>`
  without expanding them.
- `roArray.Join` with a non-string element prints a **non-fatal** diagnostic
  (`BRIGHTSCRIPT: ERROR: roArray.Join: non-string value at index 0: <file>(<line>)`)
  and returns `""`; execution continues.
- Indexing past the end returns `invalid`.
- **Known gap:** `FOR EACH` over an AA follows the device's internal hash order
  (inserted `b, a, zed, c, D` iterate as `a, D, c, zed, b`). The emulator uses
  insertion order, so this probe fails until the order is reverse-engineered.

## Runtime errors

Probes: `language/runtime-error`, `language/strings`.

An unhandled error prints the BrightScript Micro Debugger dump:

```
BrightScript Micro Debugger.
Enter any BrightScript statement, debug commands, or HELP.


Current Function:
002:  sub Main()
006:*     result = value + 1
...
Source Digest(s):
pkg: dev <major.minor.build> <md5[0..8]> <title>

<message> (runtime error &h<code>) in pkg:/source/main.brs(<line>)
Backtrace:
#0  Function main() As Void
   file/line: pkg:/source/main.brs(<line>)
Local Variables:
global           Interface:ifGlobal
m                roAssociativeArray refcnt=2 count:0
value            String (VT_STR_CONST) val:"text"
result           <uninitialized>

Brightscript Debugger>
```

- Source window: lines `max(function start, line - 8)` to
  `min(function end, line + 4)`, each formatted as `NNN:` followed by `*` on the
  failing line or a space otherwise.
- The digest is the first 8 hex digits of the MD5 of the sideloaded zip.
- Local names are padded to 17 columns.
- Error codes in use (`error.rs`): `&h18` type mismatch, `&h14` divide by
  zero, `&hf4` member function not found, `&he0` not a function, `&hf1` wrong
  argument count, `&hec` invalid dot access, `&heb` invalid index, `&h28` user
  `THROW`. Only `&h18` and `&hf4` are proven by goldens so far.
- **Provisional:** strings show `VT_STR_CONST` (literal) vs `VT_STR`
  (computed); only the literal case has a golden so far.

## Program start

Observed in the raw device console (goldens keep only the text between the
probe markers, so this is not diffed yet).

- Before `Main` runs the console shows
  `------ Running dev '<manifest title>' main ------`.
- The entry point is `Main` or `RunUserInterface`; if it declares a parameter it
  receives an AA of launch arguments (empty for now).

## Console text and comparisons

- `PRINT` column zones count **UTF-8 bytes**, not characters (`runtime/datetime`:
  `"março"` advances the column by 6).
- Comparing `invalid` with any type through `=`/`<>` is legal (not-equal);
  other operators still raise a type mismatch (`runtime/device-info`).
- Calling an unknown function name does not fail at compile time: the name
  becomes a local variable and the call raises `&he0` at run time
  (`StrToI` does not exist on OS 16).

## Runtime components (F2)

| Probe                  | Device behavior                                                                                                                                                              |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime/message-port` | `GetMessage`/`PeekMessage` return `invalid` when empty; `wait(ms, port)` returns `invalid` on timeout.                                                                       |
| `runtime/json`         | `FormatJson` sorts keys byte-wise and escapes non-ASCII as uppercase `\uXXXX`. `ParseJson` errors print `BRIGHTSCRIPT: ERROR: ParseJSON: Unknown identifier '<tok>': <loc>`. |
| `runtime/appinfo`      | Deprecated methods warn: `BRIGHTSCRIPT: WARNING: roAppInfo.GetSubtitle: This function is deprecated: <loc>`.                                                                 |
| `runtime/registry`     | `Delete` always returns `true`.                                                                                                                                              |
| `runtime/datetime`     | Names follow the device locale (`pt_BR`).                                                                                                                                    |
| `runtime/bytearray`    | `ToHexString` is uppercase; `GetSignedByte`, CRC32 and base64 match.                                                                                                         |
| `runtime/list-misc`    | `roList` prints with `(`/`)`. Unknown classes: `BRIGHTSCRIPT: ERROR: Runtime: unknown classname "X": <loc>` and `CreateObject` returns `invalid`.                            |
| `runtime/xml`          | `roXMLElement` parse, `@attr`, `GetNamedElements`, `GetText`.                                                                                                                |
| `runtime/device-info`  | `GetModel()` = `H000X` on this firmware build; display size/mode from the profile.                                                                                           |

Wrapper objects: numbers, strings and booleans read from node fields are
**boxed** (`roInt`, `roFloat`, `roString`, `roBoolean`); `type()` reports the
boxed name, operators unbox transparently.

## SceneGraph (F3)

Probes: `scenegraph/{lifecycle,node-api,observers,focus-keys,task-timer,label-basic}`.

- `CreateScene` returns at once; the scene's children and `init()` run on the
  render thread when `Show()` is called (or once `Main` blocks).
- A component's XML `<children>` are created and initialized **before** its own
  `init()`. XML attributes are applied after the child's `init()`, so they
  trigger `onChange` observers.
- Each `extends` level runs its own `init()`, base first. A component's scope
  holds the scripts of its whole chain; derived functions override base ones.
- Interface `onChange` fires before `observeField` callbacks, which fire in
  registration order. Setting an equal value does not notify unless the field
  has `alwaysNotify="true"`.
- A Task that sets a node field blocks until the render thread has run the
  observers (rendezvous).
- `isSubtype(t)` is strict (false for the node's own type).
- `Scene` has one internal background `Poster` child: `getChildCount()` counts
  it.
- `onKeyEvent` receives lowercase key names except `"OK"`; each key delivers a
  press and a release. Unhandled `back` closes the screen when
  `backExitsScene` is true.
- `LargeBoldSystemFont` Label height is 46 px (FHD).
- Node print format: `<Component: roSGNode:Type> =`, invalid fields print as
  `<Component: roInvalid>`; fields are listed most-derived type first.
- `Rectangle.color` is stored as a signed 32-bit integer (`0xRRGGBBAA`).
- Setting `Timer.control = "start"` again restarts the timer even though the
  field still reads `"start"` after it fired (`video/drm` restarts one stop
  timer per phase).

## Video (F6)

Probes: `video/{lifecycle,drm}`.

- `play` reports `buffering` after the assignment returns, then `playing`;
  `duration` is set first. `stop` reports `stopped` during the assignment.
- HTTP errors: `errorCode` -1, `errorMsg` "There was an error in the HTTP
  response. ...", `errorInfo.category` "http", `errcode` 2, `source`
  "buffer:reader"; then `error` and `finished`.
- DRM errors: `errorCode` -6, `errorMsg` "Protected content license error.",
  `errorStr` "buffer:loop:unset:DRM exception: <reason>", where the reason is
  "license url missing" (protected stream without `drmParams`) or
  "Http error <status>" (status of the license request);
  `errorInfo.category` "drm", `errcode` 9, `source` "buffer:loop:unset".
- `errorInfo` also carries `clipid` (contents started by the app),
  `dbgmsg` (= `errorStr`), `drmerrcode` 0 and `ignored` false.
- `ContentNode.drmParams` defaults to an empty associative array.
