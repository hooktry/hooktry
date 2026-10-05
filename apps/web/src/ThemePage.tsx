import { useState } from "react";
import { Patch, Riffle, Slow, Terminal } from "@lucasmarkes/hairline/react";

export function ThemePage({ theme, creating, error, onStart, onWorkspace }: {
  theme: "light" | "dark";
  creating: boolean;
  error: string | null;
  onStart: () => void;
  onWorkspace: () => void;
}) {
  const [read, setRead] = useState("rest");
  return (
    <article className="theme-page">
      <div className="theme-page-heading">
        <div><span className="eyebrow">02 / THEME</span><h1>A quieter kind of interface.</h1></div>
        <a className="text-link" href="https://hairline.lucasmarkes.com/inspo" target="_blank" rel="noreferrer">Hairline reference</a>
      </div>

      <section className="concept-home" aria-label="Hooktry concept homepage">
        <header className="concept-nav">
          <span className="concept-wordmark">hooktry<span className="concept-version"> / concept</span></span>
          <nav aria-label="Concept homepage navigation"><a href="#how-it-works">How it works</a><a href="#illustration-lab">Illustration lab</a></nav>
        </header>
        <div className="concept-hero">
          <div className="concept-hero-copy">
            <span className="eyebrow">A WEBHOOK, WITHOUT THE GUESSWORK</span>
            <h2>Every request.<br />A little more <em>clear.</em></h2>
            <p>Give your integration somewhere to send. See exactly what arrived, while it happens.</p>
            <div className="concept-actions">
              <button className="button primary" onClick={onStart} disabled={creating}>{creating ? "Creating…" : "Try a Hook"}</button>
              <button className="button secondary" onClick={onWorkspace}>Open workspace</button>
            </div>
            <span className="concept-note">No account required. Five days to explore.</span>
            {error ? <p className="error-inline" role="alert">{error}</p> : null}
          </div>
          <figure className="concept-hero-figure">
            <div className="figure-readout"><span>FIG. 01 / RECEIVE</span><span aria-live="polite">{read}</span></div>
            <Riffle intensity={0.4} theme={theme} onRead={setRead} label="A tray of request cards. Move the pointer or press the arrow keys to pull a card." />
            <figcaption>Everything lands in one place.</figcaption>
          </figure>
        </div>

        <div className="concept-sample" aria-label="Example request, demonstration only"><span className="eyebrow">EXAMPLE REQUEST</span><code>POST /orders.created</code><span>application/json</span><code>200 OK</code></div>

        <section id="how-it-works" className="concept-features" aria-label="How Hooktry works">
          <div className="concept-feature"><Slow intensity={0.4} theme={theme} label="Request parcels on a conveyor that slows under the pointer." /><span className="eyebrow">01 / RECEIVE</span><h3>One URL. Ready to listen.</h3><p>Create a public endpoint and send your first request. No server to provision.</p></div>
          <div className="concept-feature"><Terminal intensity={0.4} theme={theme} label="A terminal history with rows that lift under the pointer." /><span className="eyebrow">02 / INSPECT</span><h3>Read the whole story.</h3><p>Body, query, headers and exact timestamps. The details stay together.</p></div>
          <div className="concept-feature"><Patch intensity={0.4} theme={theme} label="A patch panel whose cables respond to the pointer." /><span className="eyebrow">03 / CONNECT</span><h3>Bring your agent along.</h3><p>Use the same interaction evidence in your browser and through MCP.</p></div>
        </section>
        <footer className="concept-footer"><span>hooktry / observe the boundary</span><span>A homepage concept. Example data.</span></footer>
      </section>

      <section className="illustration-lab" id="illustration-lab">
        <div className="lab-copy"><span className="eyebrow">03 / ILLUSTRATION LAB</span><h2>Requests, on the desk.</h2><p>A new figure made with the Hairline creation skill. Move over a tray to lift its cards. Use the slider to change how the motion spreads.</p><a className="text-link" href={`/hairline-requests.html?theme=${theme}`} target="_blank" rel="noreferrer">Open the standalone figure</a><p className="lab-credit">Figures and engine: <a href="https://hairline.lucasmarkes.com/skill" target="_blank" rel="noreferrer">Hairline by Lucas Marques</a>, MIT.</p></div>
        <iframe className="requests-bench" src={`/hairline-requests.html?theme=${theme}`} title="Interactive request trays with intensity and theme controls" loading="lazy" />
      </section>
    </article>
  );
}
