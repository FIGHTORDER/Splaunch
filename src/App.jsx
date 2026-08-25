import React from "react";
import SplaunchScreen from "./screens/SplaunchScreen.jsx";
import ErrorBoundary from "./ErrorBoundary.jsx";
import { inTauri } from "./net/connection.ts";
import {
  maps as loadMaps, launchPreview, gameInfo, units as loadUnits,
  testScenario, onGame,
} from "./net/splaunch.ts";

/* Splaunch is one screen, so this is the whole application: find the install,
   fetch the map catalogue and the roster, and hand all of it to the editor.
 *
 * It is deliberately not a lobby. There is no account, no server connection and
 * nothing to log in to - the only thing it talks to is Zero-K's public content
 * service, for the list of maps. */

export default function App() {
  const [maps, setMaps] = React.useState([]);
  const [mapsError, setMapsError] = React.useState(undefined);
  const [roster, setRoster] = React.useState({ source: "", units: [] });
  const [install, setInstall] = React.useState(undefined);
  const [installError, setInstallError] = React.useState(undefined);
  const [testError, setTestError] = React.useState(undefined);
  const [running, setRunning] = React.useState(false);

  React.useEffect(() => {
    let live = true;
    loadMaps().then(
      m => { if (live) setMaps(m); },
      e => { if (live) setMapsError(String(e?.message ?? e)); },
    );
    loadUnits().then(r => { if (live) setRoster(r); }, () => {});
    return () => { live = false; };
  }, []);

  /* Neither the engine version nor the Zero-K version is the author's to
     choose: whatever the installation has is what a scenario has to run on. So
     both are discovered. Inside the lobby the server supplied them, and when
     this was taken out of the lobby nothing replaced it - which is why Test
     could never have worked. */
  React.useEffect(() => {
    let live = true;
    Promise.all([launchPreview(), gameInfo()]).then(
      ([preview, info]) => {
        if (!live) return;
        setInstall({
          engine: preview.engine || "",
          game: preview.game || info.game || "",
          root: preview.install?.root ?? "",
          source: preview.install?.source ?? "",
          ais: info.ais ?? [],
          maps: info.maps ?? [],
          engines: info.engines ?? [],
          games: info.games ?? [],
        });
      },
      e => { if (live) setInstallError(String(e?.message ?? e)); },
    );
    return () => { live = false; };
  }, []);

  /* The engine announces itself starting and stopping. Nothing used to listen,
     so `running` was set on Test and cleared only if the launch threw: after
     one successful test the app believed a game was running forever and refused
     to start another. */
  React.useEffect(() => {
    if (!inTauri()) return undefined;
    let stop = () => {};
    let live = true;
    onGame(status => {
      if (status.kind === "launched") setRunning(true);
      if (status.kind === "exited") setRunning(false);
      if (status.kind === "failed") {
        setRunning(false);
        setTestError(status.reason);
      }
    }).then(off => { if (live) stop = off; else off(); }, () => {});
    return () => { live = false; stop(); };
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%",
      minHeight: 0, background: "var(--surface-base)", overflow: "hidden" }}>
      <ErrorBoundary>
        <SplaunchScreen
          maps={maps}
          roster={roster}
          install={install}
          player="Player"
          running={running}
          testError={testError || installError || mapsError}
          onTest={sc => {
            setTestError(undefined);
            testScenario(sc, "Player", install?.engine ?? "")
              .catch(e => setTestError(String(e?.message ?? e)));
          }}
        />
      </ErrorBoundary>
      {!inTauri() && (
        <div style={{ padding: "var(--sp-4) var(--sp-6)", borderTop: "1px solid var(--w-12)",
          font: "var(--text-ui-sm)", color: "var(--text-faint)" }}>
          Running in a browser, so nothing can be launched. Open the desktop app.
        </div>
      )}
    </div>
  );
}
