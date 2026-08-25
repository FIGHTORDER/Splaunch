/**
 * Whether we are inside the desktop shell.
 *
 * This file used to be Shiro's bridge to a TCP relay, and it came across with
 * the screen: `connect`, `sendLine`, `disconnect`, `passwordHash` and two event
 * subscriptions, all of them invoking `zks_*` commands that Splaunch's Rust
 * side does not register. Any call would have thrown at runtime. It also
 * carried the address of `zero-k.info:8200`, which is the one host the notes
 * say never to connect to for testing, because repeated failures get the IP
 * banned.
 *
 * Splaunch is not a lobby: no account, no server, nothing to log in to. The
 * only thing it talks to is Zero-K's public content service, from Rust. So all
 * that is left of this module is the one function anything actually used.
 */

/** True when running inside the Tauri shell rather than a plain browser tab. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
