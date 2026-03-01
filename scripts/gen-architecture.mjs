import { renderMermaidSVG, THEMES } from "beautiful-mermaid";
import { writeFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const diagram = `
graph TB
    CLI["<b>CLI</b><br/><i>clap</i>"]

    FE["Frecency<br/>Engine"]
    PD["Project<br/>Detect"]
    WP["Waypoints"]
    FM["Fuzzy<br/>Match"]

    DB[("<b>SQLite</b><br/><i>rusqlite · WAL mode</i>")]

    AI["AI Layer<br/><i>feature-gated</i>"]
    TUI["TUI Picker<br/><i>feature-gated</i>"]

    SHELL["<b>Shell Integration</b><br/><i>bash · zsh · fish · pwsh · nushell · elvish</i>"]

    CLI --> FE & PD & WP & FM
    FE & PD & WP & FM --> DB
    DB --> AI & TUI
    AI & TUI --> SHELL
`;

const theme = THEMES["catppuccin-mocha"];
const svg = renderMermaidSVG(diagram, { ...theme, padding: 60 });
const out = resolve(__dirname, "..", "docs", "architecture.svg");

writeFileSync(out, svg);
console.log(`Wrote ${out}`);
