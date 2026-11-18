# Sign-in (M9)

Status: **built; the default app ID was decided 2026-11-17 and the sign-in is not yet run end to end on a phone.**
Everybody signs in the way most Minecraft launchers do: a device code on `login.live.com` with the legacy app ID
`00000000402b5328`. There is no app registration, no Mojang approval to wait for, and nothing to configure.

## What it is for

Decided 2026-11-09: **sign-in gates the download of the game (M8b) and nothing else.** Playing game
files that are already on the phone never asks for it.

Decided 2026-11-10: **when someone is signed in, the game plays as that profile.** It is an identity,
not a gate: see "The game's account" below. The desktop launcher is stricter (no owner, no
Play); this port is not, because its files can also come from a computer through Game files.

The gate is a policy control, not copy protection. The engine does not enforce it.

## How it signs in

Decided 2026-11-17 (the owner): the sign-in that had been a private "test mode" is the real one, for everybody. The plan of registering
this project's own Azure app and waiting for Mojang's approval is dropped, because most launchers sign in with the same app ID.

**The app ID.** `HZDefaultClientID` in `AccountModel.m`: `00000000402b5328`, a legacy Microsoft app ID. A public client ID is not a secret.
A player can use another one (Settings > Developer options > "Use a different app ID", or the file `Documents/Horizon/app-id.txt`, or the
environment variable `HORIZON_MSA_CLIENT_ID`; the first version's `test-client-id.txt` is still read). It is kept on the phone only, never in a
build, never logged. "Go back to the built-in app ID" removes it.

**What the requests are** (all in `AccountClient.m`, one path, no modes):
- device code and token requests go to `login.live.com/oauth20_connect.srf` and `oauth20_token.srf` (with `response_type=device_code`);
- scope `service::user.auth.xboxlive.com::MBI_SSL`;
- the Xbox ticket is the bare token, with no `d=` in front;
- then Xbox Live, XSTS, `login_with_xbox`, the entitlements and the profile, as in `horizon/docs/microsoft-account.md`.

**What was checked (2026-11-10, live against Microsoft).** This ID is a *legacy* Microsoft app: the v2 endpoint refuses it (`AADSTS700016`), and the
legacy device-code endpoint accepts it and returns a code for `microsoft.com/link`. Polling answers `authorization_pending`; a bad refresh token
answers `invalid_grant`. Same field names as v2, so the same parser reads them.
**Not checked:** a real sign-in end to end, Xbox Live accepting the bare token, and Mojang's `login_with_xbox` returning a token for it. Only a real
sign-in on a phone settles these (see the end of this page).

**The risk, plainly.** The ID belongs to the official Minecraft Launcher family. Microsoft or Mojang can block it for this app (or for everyone using
it) at any time, and Mojang's guidance for third-party launchers is to register your own. If it is blocked the Minecraft services answer 403 and
the Account screen says so (`HZAppNotApproved`); the way out is the "different app ID" field, or registering an app after all. The sign-in code
needs no change for either.

## The game's account

The engine takes an account from a *session file* (`--session-file`, `AuthSession` in
`horizon-client-render/src/auth.rs`): JSON with the profile name, UUID and Minecraft access token.
With one the game plays under that name and UUID and can join online-mode servers; without one it
plays offline under a generated name, as before.

`main.m` writes that file just before it starts the engine (`GameSession.m`) and adds
`--session-file <path>` to the engine's arguments, only when an account is saved. The engine's own
rules, each checked by `platform/ios/tests/game_session_check.py` against a copy of its parser:

- **Read once and deleted** by the engine, so the token does not stay on the disk. A leftover from a
  run that ended before the engine started is deleted at the next launch.
- **Refused if older than 120 seconds**, which is why it is written in `main()` and not earlier.
- **Refused if group or others can read it**, so it is created with mode 0600.
- Name 1 to 16 characters, UUID 32 hex digits, token not empty. Anything less and the file is not
  written, and the game starts offline.

**What this does not do.** It never blocks playing: a signed-in account that is offline, expired or
not confirmed as an owner still starts the game. Owning the game gates only the download.

**Token freshness.** The Minecraft token lasts about a day. It is renewed (`recheck`) silently once
each time the launcher opens, and at sign-in. The game run does not renew it: with "Open straight
into the game" on, the launcher is skipped, so the token is as old as the last launcher visit. A stale
token makes online servers refuse the player. Singleplayer and LAN do not use it.

**Not verified on a phone**, and not verifiable here: that the engine accepts the file on iOS
(the Unix permission check should apply, as iOS is Unix), that the in-game name and skin become the
profile's, and that an online server accepts the token. The first check is the name on the Account
button and, in the game, the player name.

## The files

| File | What it does |
|---|---|
| `AccountModel.h/.m` | The rules, with no network and no UIKit: reading each reply, the refusal messages (the desktop launcher's words), whether the account owns Java Edition, and `HZMayDownload`, the one place the gate lives. |
| `AccountClient.h/.m` | The requests, in the order of the doc: device code, poll, Xbox Live, XSTS, `login_with_xbox`, entitlements, profile. An ephemeral `NSURLSession` (no cookies, no cache, nothing on disk). No body is ever logged. |
| `AccountStore.h/.m` | The Keychain (two items) and `<Documents>/Horizon/account.txt` (name, UUID, last answer; no secret). |
| `GameSession.h/.m` | Hands the signed-in account to the engine as its session file (below). Called from `main.m`. |
| `LauncherAccount.h/.m` | The Account screen, reached from a new *Account* button on the launcher. |

## Decisions

- **Device code**, as on desktop. It needs no redirect URI. On a phone
  the player reads a code on the same device that shows the page, so the code is copied to the clipboard
  and the page opens with one tap. A web-authentication sheet with a custom redirect is an alternative
  to evaluate once the app exists.
- **`login_with_xbox` only.** The desktop launcher tries `launcher/login` (platform `PC_LAUNCHER`) first,
  because a token from `login_with_xbox` alone makes joining an online-mode server fail with "Profile
  not found". This port does not claim to be a PC launcher, so it does not use that route. **Online
  servers (roadmap M10) will need the question reopened.**
- **Keychain class** `AfterFirstUnlockThisDeviceOnly`: no backup, no other device. The Keychain takes
  long items, so the desktop's splitting workaround is not needed.
- **Offline never blocks someone who passed before.** `HZMayDownload`: a fresh answer wins (a fresh "no"
  blocks at once, even after a past "yes"); with no network the last answer stands; no account, or never
  a yes, blocks.
- **The refresh token is renewed** at each check (`recheck`), and a refused one (`invalid_grant`) asks
  the player to sign in again.
- **Sign out** deletes both Keychain items and `account.txt`.

## What is not done

- **The download itself (M8b)** does not exist, so nothing calls `HZMayDownload` yet. The gate is built
  and its rules are tested; the thing it guards is not.
- **Sign in with Apple.** The roadmap (D12) notes that Apple may expect it beside a third-party login
  for the App Store. This app is not distributed through Apple, so it is not added.
- **Skin and cape** of the profile are not read. The profile screen shows the name only.
- **Token expiry timers.** The Minecraft token (24 hours) is renewed when a check runs, not on a timer.
- **Not compiled or run.** There is no Objective-C compiler where this was written. CI's
  *iOS shell check* compiles each `.m` against the iOS SDK; that is the first compile.

## How it is tested

`platform/ios/tests/account_rules_check.py` checks the decisions of `AccountModel.m` against the reply
shapes in the desktop launcher's own tests (`horizon/launcher/src/account/tests.rs`): a Python copy of
the logic, as `SessionLog` has a C copy of its line format. It checks the logic, not the Objective-C
text, so it does not replace the CI compile.

## To check on a phone

1. Account, Sign in: a code appears and is on the clipboard.
2. Open the page, type the code, finish in the browser: the screen shows your Minecraft name.
3. An account that does not own Java Edition: "This account doesn't own Minecraft: Java Edition."
4. Airplane mode after a pass: the account still shows as an owner.
5. Sign out, then check the Keychain item is gone (Settings has none to show; sign in again to see the
   code screen return).
6. A network trace shows no token in any log line (Diagnostics > the game's log, and the session log).
