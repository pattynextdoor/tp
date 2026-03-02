// Fig/Warp completion spec for tp
// Place in ~/.fig/autocomplete/src/tp.ts or submit to withfig/autocomplete

const completionSpec: Fig.Spec = {
  name: "tp",
  description: "Teleport anywhere in your codebase — AI-enhanced, project-aware directory navigation",
  args: {
    name: "query",
    description: "Directory name or pattern to navigate to",
    isVariadic: true,
    isOptional: true,
    generators: {
      script: ["tp", "--complete", ""],
      postProcess: (output) =>
        output
          .trim()
          .split("\n")
          .filter(Boolean)
          .map((line) => ({
            name: line,
            description: line.startsWith(":") ? "Waypoint" : line.startsWith("@") ? "Project" : "Directory",
            icon: line.startsWith(":") ? "📌" : line.startsWith("@") ? "📁" : "📂",
          })),
    },
  },
  options: [
    {
      name: ["-i", "--interactive"],
      description: "Interactive TUI picker mode",
    },
    {
      name: ["-p", "--project"],
      description: "Project-scoped search",
    },
    {
      name: "--mark",
      description: "Create a waypoint (bookmark) for a directory",
      args: [
        { name: "name", description: "Waypoint name" },
        { name: "path", description: "Directory path (default: cwd)", isOptional: true },
      ],
    },
    {
      name: "--unmark",
      description: "Remove a waypoint",
      args: {
        name: "name",
        description: "Waypoint name to remove",
        generators: {
          script: ["tp", "--complete", ":"],
          postProcess: (output) =>
            output
              .trim()
              .split("\n")
              .filter(Boolean)
              .map((line) => ({ name: line.replace(/^:/, ""), icon: "📌" })),
        },
      },
    },
    {
      name: "--waypoints",
      description: "List all waypoints",
    },
    {
      name: "--setup-ai",
      description: "Configure AI API key",
    },
    {
      name: "--recall",
      description: "AI session recall",
    },
  ],
  subcommands: [
    {
      name: "init",
      description: "Generate shell initialization code",
      args: {
        name: "shell",
        description: "Shell to generate init for",
        suggestions: ["bash", "zsh", "fish", "powershell", "nushell", "elvish"],
      },
      options: [
        {
          name: "--cmd",
          description: "Custom command name (default: tp)",
          args: { name: "name" },
        },
      ],
    },
    {
      name: "import",
      description: "Import navigation data from another tool",
      options: [
        {
          name: "--from",
          description: "Tool to import from",
          isRequired: true,
          args: {
            name: "tool",
            suggestions: ["zoxide", "z", "autojump"],
          },
        },
      ],
      args: {
        name: "path",
        description: "Path to database file (auto-detected if omitted)",
        isOptional: true,
        template: "filepaths",
      },
    },
    {
      name: "add",
      description: "Record a directory visit (called by shell hooks)",
      args: { name: "path", template: "folders" },
    },
    {
      name: ["ls", "list"],
      description: "List top directories by frecency score",
      options: [
        {
          name: ["-n", "--count"],
          description: "Number of entries to show (default: 20)",
          args: { name: "count", default: "20" },
        },
      ],
    },
    {
      name: "back",
      description: "Jump back in navigation history",
      args: {
        name: "steps",
        description: "How many steps back (default: 1)",
        isOptional: true,
        default: "1",
      },
    },
    {
      name: "completions",
      description: "Generate shell completions",
      args: {
        name: "shell",
        suggestions: ["bash", "zsh", "fish", "powershell", "elvish", "nushell"],
      },
    },
    {
      name: "sync",
      description: "Cloud sync (Pro feature)",
    },
  ],
};

export default completionSpec;
