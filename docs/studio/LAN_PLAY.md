# Two-player LAN conversation

The opt-in Studio LAN mode keeps all world state, AI calls, voice engines, assets and
spending on the host computer. A second device needs only a browser on the same private
network. The initial guest view is typed conversation; microphone capture and model
configuration stay on the host. Browser microphone access over plain LAN HTTP is not
assumed. This example supports one host (`player_1`) and one guest (`player_2`).

Start the usual Web host with `--lan PRIVATE_IPV4`, using the host's actual private IPv4
interface. The original loopback URL remains the host/admin UI. A second listener binds
only the selected LAN address at the same port. This is private-LAN HTTP, without TLS,
public-address binding, automatic router forwarding, Internet hosting or reconnectable
persistent identities. On systems with a firewall, allow the chosen port on the private
network if needed. The current LAN implementation uses `/dev/urandom` on Unix; a missing OS random source fails closed.

In **Settings → Play together**, copy the guest URL and session code to your friend.
They open `/join`, enter the code, and receive a fixed player-2 identity. The code never
appears in the guest URL. Cookies are HttpOnly and SameSite=Strict; server checks the exact
listener Host/Origin plus custom JSON request headers. Five failed join attempts block
further attempts for one minute. There is one guest slot. **Disconnect guest / New
invitation** revokes the guest token, changes the code, and releases an unfinished guest
turn without pretending its unplayed text was heard. Restarting the host revokes sessions.

The guest can send conversation to a present merchant or keeper and acknowledge reading
its own reply. The host retains travel, inventory/trade actions, promises, saves, asset
approval, voice, provider and spending controls. A guest cannot switch player IDs, fetch
host state/inspectors/configuration, issue a world command, or acknowledge player 1's
hearing. All guest text is bounded and passed through the same host context compiler;
models still cannot execute game actions. A shared floor prevents overlapping host/guest
turns from replacing another player's pending reply. Messages keep explicit speaker and
recipient attribution. NPC attitudes and episodic context are selected for the addressing
player, not the host's currently selected player. The host identity remains player 1.

The guest transcript starts at join. It contains new player lines and host-confirmed NPC
speech prefixes in the shared room, not old private inspector data or unheard speech
tails. The guest's current reply is displayed for review; **I've read this reply** confirms
that complete text to the host. Browsers poll at 600 ms; rendering performs no inference.
The host can restore/reset the world; clients refresh against the current generation and
stale actions are rejected. Approved artwork remains managed by the host; the lightweight
guest UI currently focuses on text, not replicated asset/voice playback.

Offline evidence: `eval/studio/lan-session-host.json` covers ten authorization/dialogue workflows;
`eval/studio/lan-session-model-mock.json` records four AI requests on the host with alternating
player-1/player-2 subjects. `eval/studio/voice-lan-browser.json` covers ten real Firefox Settings
and two-origin workflows. Tests use loopback addresses as LAN substitutes and cap zero;
they do not measure Wi-Fi reliability, WAN exposure, TLS, live model quality or microphone
accuracy. Earlier conformance/evaluation outputs remain frozen.
