use anyhow::{bail, Result};

/// Generate shell initialization code for the given shell.
///
/// The generated code creates a shell function that wraps the `tp` binary:
/// - Captures stdout (which is the target directory path)
/// - If the output is a directory, `cd` into it
/// - Installs a chpwd/precmd hook to record visits via `tp add`
pub fn generate_init(shell: &str, cmd: &str) -> Result<String> {
    match shell {
        "bash" => Ok(generate_bash(cmd)),
        "zsh" => Ok(generate_zsh(cmd)),
        "fish" => Ok(generate_fish(cmd)),
        "powershell" | "pwsh" => Ok(generate_powershell(cmd)),
        "nushell" | "nu" => Ok(generate_nushell(cmd)),
        "elvish" => Ok(generate_elvish(cmd)),
        _ => bail!(
            "unsupported shell: {}. Supported: bash, zsh, fish, powershell, nushell, elvish",
            shell
        ),
    }
}

fn generate_bash(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for bash
{cmd}() {{
    local result
    result="$(command tp "$@")"
    if [ -d "$result" ]; then
        builtin cd -- "$result"
    elif [ -n "$result" ]; then
        echo "$result"
    fi
}}

# Record directory changes in the background
__tp_hook() {{
    command tp add -- "$PWD" &>/dev/null &
    disown &>/dev/null
}}

if [[ ";${{PROMPT_COMMAND[*]}};" != *";__tp_hook;"* ]]; then
    PROMPT_COMMAND="${{PROMPT_COMMAND:+$PROMPT_COMMAND;}}__tp_hook"
fi

# Dynamic tab completion
__{cmd}_completions() {{
    local cur="${{COMP_WORDS[COMP_CWORD]}}"
    if [[ "$cur" == :* ]] || [[ "$cur" == @* ]] || [[ -n "$cur" ]]; then
        COMPREPLY=($(command tp --complete "$cur" 2>/dev/null))
    else
        COMPREPLY=($(command tp --complete "" 2>/dev/null))
    fi
}}
complete -F __{cmd}_completions {cmd}
"#,
        cmd = cmd
    )
}

fn generate_zsh(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for zsh
{cmd}() {{
    local result
    result="$(command tp "$@")"
    if [[ -d "$result" ]]; then
        builtin cd -- "$result"
    elif [[ -n "$result" ]]; then
        echo "$result"
    fi
}}

# Record directory changes in the background
__tp_hook() {{
    command tp add -- "$PWD" &>/dev/null &!
}}

[[ -z "${{precmd_functions[(r)__tp_hook]}}" ]] && precmd_functions+=(__tp_hook)

# Dynamic tab completion
__{cmd}_completions() {{
    local -a completions
    completions=(${{(@f)"$(command tp --complete "${{words[CURRENT]}}" 2>/dev/null)"}})
    compadd -a completions
}}
compdef __{cmd}_completions {cmd}
"#,
        cmd = cmd
    )
}

fn generate_fish(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for fish
function {cmd}
    set -l result (command tp $argv)
    if test -d "$result"
        builtin cd -- "$result"
    else if test -n "$result"
        echo "$result"
    end
end

# Record directory changes in the background
function __tp_hook --on-variable PWD
    command tp add -- "$PWD" &>/dev/null &
end

# Dynamic tab completion
complete -c {cmd} -f -a '(command tp --complete (commandline -ct) 2>/dev/null)'
"#,
        cmd = cmd
    )
}

fn generate_powershell(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for PowerShell
function {cmd} {{
    $result = & (Get-Command tp -CommandType Application | Select-Object -First 1) @args
    if (Test-Path $result -PathType Container) {{
        Set-Location $result
    }} elseif ($result) {{
        Write-Output $result
    }}
}}

# Record directory changes
$ExecutionContext.InvokeCommand.LocationChangedAction = {{
    Start-Process -NoNewWindow -FilePath tp -ArgumentList "add","--","$PWD" *>$null
}}
"#,
        cmd = cmd
    )
}

fn generate_nushell(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for nushell
def --env {cmd} [...args: string] {{
    let result = (^tp ...$args | str trim)
    if ($result | path exists) and ($result | path type) == "dir" {{
        cd $result
    }} else if ($result | is-not-empty) {{
        print $result
    }}
}}
"#,
        cmd = cmd
    )
}

fn generate_elvish(cmd: &str) -> String {
    format!(
        r#"# tp shell integration for elvish
fn {cmd} {{|@args|
    var result = (external tp $@args)
    if (path:is-dir $result) {{
        cd $result
    }} elif (not-eq $result "") {{
        echo $result
    }}
}}

set after-chdir = [$@after-chdir {{|dir|
    external tp add -- $dir &>/dev/null &
}}]
"#,
        cmd = cmd
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bash() {
        let code = generate_init("bash", "tp").unwrap();
        assert!(code.contains("tp()"));
        assert!(code.contains("__tp_hook"));
        assert!(code.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_generate_zsh() {
        let code = generate_init("zsh", "tp").unwrap();
        assert!(code.contains("tp()"));
        assert!(code.contains("precmd_functions"));
    }

    #[test]
    fn test_generate_fish() {
        let code = generate_init("fish", "tp").unwrap();
        assert!(code.contains("function tp"));
        assert!(code.contains("--on-variable PWD"));
    }

    #[test]
    fn test_generate_powershell() {
        let code = generate_init("powershell", "tp").unwrap();
        assert!(code.contains("function tp"));
    }

    #[test]
    fn test_generate_nushell() {
        let code = generate_init("nushell", "tp").unwrap();
        assert!(code.contains("def --env tp"));
    }

    #[test]
    fn test_generate_elvish() {
        let code = generate_init("elvish", "tp").unwrap();
        assert!(code.contains("fn tp"));
    }

    #[test]
    fn test_custom_command_name() {
        let code = generate_init("bash", "j").unwrap();
        assert!(code.contains("j()"));
        assert!(!code.contains("tp()"));
    }

    #[test]
    fn test_unsupported_shell() {
        let result = generate_init("tcsh", "tp");
        assert!(result.is_err());
    }

    #[test]
    fn test_pwsh_alias() {
        let code = generate_init("pwsh", "tp").unwrap();
        assert!(code.contains("function tp"));
    }

    #[test]
    fn test_nu_alias() {
        let code = generate_init("nu", "tp").unwrap();
        assert!(code.contains("def --env tp"));
    }
}
