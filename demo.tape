# tp demo — 30 second showcase
# Run: vhs demo.tape

Output demo.gif

Require tp

Set Shell "zsh"
Set FontSize 16
Set Width 900
Set Height 550
Set Padding 20
Set Theme "synthwave"
Set TypingSpeed 50ms
Set PlaybackSpeed 1.0

# --- Setup (hidden): create fake project dirs + seed tp ---
Hide
Type "setopt INTERACTIVE_COMMENTS"
Enter
Sleep 200ms
# Create fake project dirs with .git markers so project-scoping works
Type `mkdir -p ~/code/payments-service/.git ~/code/payments-service/tests ~/code/payments-service/src/api ~/code/payments-frontend/.git ~/code/payments-frontend/src/components ~/code/tp/.git ~/infra/k8s/deploy`
Enter
Sleep 500ms
Type "source /opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh"
Enter
Sleep 200ms
Type `eval "$(tp init zsh)"`
Enter
Sleep 500ms
# Set prompt AFTER tp init so it's not overridden
Type `export PS1='%F{cyan}%~ %F{white}❯ %f'`
Enter
Sleep 200ms
# Seed directories so tp has frecency data
Type "tp add ~/code/payments-service"
Enter
Sleep 200ms
Type "tp add ~/code/payments-service/tests"
Enter
Sleep 200ms
Type "tp add ~/code/payments-service/src/api"
Enter
Sleep 200ms
Type "tp add ~/code/payments-frontend"
Enter
Sleep 200ms
Type "tp add ~/code/payments-frontend/src/components"
Enter
Sleep 200ms
Type "tp add ~/code/tp"
Enter
Sleep 200ms
Type "tp add ~/infra/k8s/deploy"
Enter
Sleep 200ms
# Mark waypoints
Type "tp --mark deploy ~/infra/k8s/deploy"
Enter
Sleep 200ms
Type "tp --mark api ~/code/payments-service/src/api"
Enter
Sleep 200ms
Type "tp --mark ui ~/code/payments-frontend/src/components"
Enter
Sleep 200ms
# Start from home
Type "cd ~"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 300ms
Show

# --- Beat 1: Fuzzy jump ---
Sleep 500ms
Type `echo "▸ Jump anywhere with fuzzy matching"`
Enter
Sleep 500ms

Type "tp payments"
Sleep 300ms
Enter
Sleep 2s

# Blank line between sections
Type "echo"
Enter
Sleep 500ms

# --- Beat 2: Project switch ---
Type `echo "▸ Switch projects instantly"`
Enter
Sleep 500ms

Type "tp @payments-frontend"
Sleep 300ms
Enter
Sleep 2s

# Blank line between sections
Type "echo"
Enter
Sleep 500ms

# --- Beat 3: Cross-repo deep jump ---
Type `echo "▸ Jump across repos to any subdirectory"`
Enter
Sleep 500ms

Type "tp api"
Sleep 300ms
Enter
Sleep 2s

# Blank line between sections
Type "echo"
Enter
Sleep 500ms

# --- Beat 4: Waypoints ---
Type `echo "▸ Pinned waypoints for paths you always need"`
Enter
Sleep 500ms

Type "tp :deploy"
Sleep 300ms
Enter
Sleep 2s

# Blank line between sections
Type "echo"
Enter
Sleep 500ms

# --- Beat 5: Backtrack ---
Type `echo "▸ Retrace your steps"`
Enter
Sleep 500ms

Type "tp back"
Sleep 300ms
Enter
Sleep 2s

# Clear screen before the finale
Type "clear"
Enter
Sleep 500ms

# --- Beat 6: Waypoint list ---
Type `echo "▸ Your pinned waypoints"`
Enter
Sleep 500ms

Type "tp --waypoints"
Sleep 300ms
Enter
Sleep 3s

Sleep 1.5s

# --- Cleanup (hidden): remove fake dirs ---
Hide
Type `rm -rf ~/code/payments-service ~/code/payments-frontend ~/code/tp ~/infra`
Enter
Sleep 300ms
