# tp demo — 30 second showcase
# Run: vhs demo.tape

Output demo.gif

Require tp

Set FontSize 16
Set FontFamily "JetBrains Mono"
Set Width 900
Set Height 500
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 50ms
Set PlaybackSpeed 1.0

# --- Setup (hidden): ensure shell integration is loaded ---
Hide
Type "eval \"$(tp init zsh)\""
Enter
Sleep 500ms
# Seed some directories so the demo looks real
Type "tp add ~/code/payments-service"
Enter
Sleep 200ms
Type "tp add ~/code/payments-service/tests"
Enter
Sleep 200ms
Type "tp add ~/code/payments-service/src/api"
Enter
Sleep 200ms
Type "tp add ~/code/frontend"
Enter
Sleep 200ms
Type "tp add ~/code/frontend/src/components"
Enter
Sleep 200ms
Type "tp add ~/code/tp"
Enter
Sleep 200ms
Type "tp add ~/infra/k8s/deploy"
Enter
Sleep 200ms
# Mark a waypoint
Type "tp --mark deploy ~/infra/k8s/deploy"
Enter
Sleep 200ms
# Start from home
Type "cd ~"
Enter
Sleep 200ms
Show

# --- Beat 1: Fuzzy jump ---
Sleep 500ms
Type "# Jump anywhere with fuzzy matching"
Sleep 500ms
Enter
Sleep 800ms

Type "tp payments"
Sleep 300ms
Enter
Sleep 1.5s

# --- Beat 2: Project-scoped search ---
Type "# Stay inside your project"
Sleep 500ms
Enter
Sleep 800ms

Type "tp -p tests"
Sleep 300ms
Enter
Sleep 1.5s

# --- Beat 3: Cross-project switch ---
Type "# Switch projects instantly"
Sleep 500ms
Enter
Sleep 800ms

Type "tp @frontend"
Sleep 300ms
Enter
Sleep 1.5s

# --- Beat 4: Waypoints ---
Type "# Pinned waypoints for paths you always need"
Sleep 500ms
Enter
Sleep 800ms

Type "tp :deploy"
Sleep 300ms
Enter
Sleep 1.5s

# --- Beat 5: Backtrack ---
Type "# Retrace your steps"
Sleep 500ms
Enter
Sleep 800ms

Type "tp back"
Sleep 300ms
Enter
Sleep 1.5s

# --- Beat 6: Styled list ---
Type "# See where you've been"
Sleep 500ms
Enter
Sleep 800ms

Type "tp ls"
Sleep 300ms
Enter
Sleep 2.5s

Sleep 1s
