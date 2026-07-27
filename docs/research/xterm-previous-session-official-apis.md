# Previous xterm-backed view: official API findings

## Question and versions checked

Does React, Tauri v2, or xterm.js already track the previously active agent, terminal, or
command view so an application can select it when the current view closes?

The repository currently resolves React 19.2.7, `@tauri-apps/api` 2.11.0, and
`@xterm/xterm` 6.0.0. The findings below use only the projects' official documentation and
public API declarations.

## Result

No documented API in React, Tauri v2, or xterm.js provides terminal/session activation history,
a "previous active" view, or MRU ordering. Their focus APIs observe or apply focus at different
layers; they do not remember Soloist's semantic view-selection history.

Consequently, previous-view selection must be application state keyed by Soloist view/process
IDs. The application should explicitly record semantic activations, maintain its own eligibility
policy by forgetting explicit stop/removal targets and IDs that disappear, select the most recently
active still-present eligible pane when the current pane is explicitly stopped or removed, and show
the starter panel when no eligible prior pane remains. A natural process exit does not itself alter
that history or trigger navigation. This conclusion is an inference from the documented API
boundaries below, not behavior supplied by any of the three libraries.

## React

- React DOM exposes `onFocus` and `onBlur`. Both bubble in React, so a pane wrapper can observe
  focus moving into or out of its subtree. A React `FocusEvent` also includes `relatedTarget`,
  which describes the counterpart of that particular focus transition; it is not a retained
  activation history. See the official [focus event reference](https://react.dev/reference/react-dom/components/common#focus-event-handler)
  and [focus-subtree example](https://react.dev/reference/react-dom/components/common#handling-focus-events).
- React documents `useRef` as a way to retain a mutable value between renders and `useState` as a
  way to retain rendered component state. Either can hold history owned by the application, but
  React does not create or interpret that history. See [`useRef`](https://react.dev/reference/react/useRef#referencing-a-value-with-a-ref)
  and [`useState`](https://react.dev/reference/react/useState).
- Therefore DOM focus events can be an input signal, but they are not a complete source of truth
  for view activation: an application can select a view programmatically without producing a new
  focus transition. The semantic selection action/state should record the activation directly.

## Tauri v2

- `Window.isFocused()` returns only the current focus state of a native Tauri window, while
  `Window.onFocusChanged()` reports a boolean when that native-window focus changes. Neither
  includes a previous-window identifier or history. See
  [`isFocused`](https://v2.tauri.app/reference/javascript/api/namespacewindow/#isfocused) and
  [`onFocusChanged`](https://v2.tauri.app/reference/javascript/api/namespacewindow/#onfocuschanged).
- `Window.setFocus()` focuses a native window. `Webview.setFocus()` brings a Tauri webview to the
  front and focuses it. These operate on Tauri containers, not on React children or xterm
  instances within one webview. See the official
  [`Window.setFocus`](https://v2.tauri.app/reference/javascript/api/namespacewindow/#setfocus) and
  [`Webview.setFocus`](https://v2.tauri.app/reference/javascript/api/namespacewebview/#setfocus)
  references.
- `getAllWindows()`, `getAllWebviews()`, and `getAllWebviewWindows()` promise lists of currently
  available containers. Their contracts do not define those lists as activation/MRU order and do
  not expose predecessor relationships. See
  [`getAllWindows`](https://v2.tauri.app/reference/javascript/api/namespacewindow/#getallwindows),
  [`getAllWebviews`](https://v2.tauri.app/reference/javascript/api/namespacewebview/#getallwebviews),
  and [`getAllWebviewWindows`](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/#getallwebviewwindows).
- At the Rust layer, `tauri::WindowEvent::Focused(bool)` likewise reports only whether a native
  window gained or lost focus. See the official
  [`WindowEvent::Focused`](https://docs.rs/tauri/latest/tauri/enum.WindowEvent.html#variant.Focused)
  reference.

Tauri focus events are relevant for app-wide behaviors such as resyncing when the desktop window
regains focus. They cannot identify which xterm-backed view inside the current React webview was
previously active.

## xterm.js 6.0.0

- The public `Terminal` class exposes [`focus()`](https://xtermjs.org/docs/api/terminal/classes/terminal/#focus)
  and [`blur()`](https://xtermjs.org/docs/api/terminal/classes/terminal/#blur) to apply or remove
  terminal input focus.
- It exposes the containing [`element`](https://xtermjs.org/docs/api/terminal/classes/terminal/#element)
  and input [`textarea`](https://xtermjs.org/docs/api/terminal/classes/terminal/#textarea) DOM
  handles. Standard DOM/React focus events can therefore observe focus around an opened terminal.
- The complete official [`Terminal` API index](https://xtermjs.org/docs/api/terminal/classes/terminal/)
  contains no `onFocus`, `onBlur`, previous-terminal, session, tab, or activation-history member.
  The versioned public declaration confirms the available `focus()` and `blur()` methods:
  [`xterm.d.ts` 6.0.0](https://github.com/xtermjs/xterm.js/blob/6.0.0/typings/xterm.d.ts#L1005-L1013).
- xterm.js defines itself as a frontend component rather than a shell, terminal application, or
  process/session manager. Process and view identity therefore belong to the host application.
  See the project's official
  ["What xterm.js is not" section](https://github.com/xtermjs/xterm.js/#what-xtermjs-is-not).

After application state selects and renders the surviving prior view, its `Terminal.focus()`
method is the appropriate final primitive for returning keyboard input to that terminal. It does
not decide which terminal is prior.

## Implementation boundary established by the APIs

The required behavior has two separate responsibilities:

1. Soloist owns an ordered activation history and eligibility policy for its agent, terminal, and
   command view IDs. When the current pane is explicitly stopped or removed, it resolves the most
   recently activated, still-present eligible pane or falls back to the starter panel. A natural
   process exit alone does not change the selection.
2. React renders that selection; once its xterm instance is visible, xterm's `focus()` restores
   keyboard focus. Tauri window/webview focus APIs are not part of selecting among xterm instances
   inside the same webview.

This history should be driven by the application's canonical "select/activate view" paths so it
covers mouse selection, keyboard navigation, creation, and programmatic selection consistently.
