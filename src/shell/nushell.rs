pub const INTEGRATION: &str = r#"# mh shell integration for nushell
if not ("MH_SESSION_ID" in $env) {
  $env.MH_SESSION_ID = $"(date now | format date '%s')-($nu.pid)"
}
if not ("MH_SKIP_GIT_DETECT" in $env) {
  $env.MH_SKIP_GIT_DETECT = "1"
}

let mh_existing_hooks = ($env.config.hooks? | default {})
$env.config = ($env.config | upsert hooks {
  pre_execution: (($mh_existing_hooks.pre_execution? | default []) ++ [{||
    let cmd = (commandline)
    if ($cmd | str trim | is-empty) { return }
    if ($cmd | str starts-with "mh ")
      or ($cmd | str starts-with "command mh ")
      or ($cmd | str starts-with "__mh_")
      or ($cmd | str starts-with "_mh_") { return }
    $env.MH_LAST_COMMAND = $cmd
    $env.MH_START_TIME = (date now | into int | $in / 1000000)
  }])
  pre_prompt: (($mh_existing_hooks.pre_prompt? | default []) ++ [{||
    if ("MH_LAST_COMMAND" in $env) {
      let exit_code = ($env.LAST_EXIT_CODE? | default 0)
      let end_time = (date now | into int | $in / 1000000)
      let duration_ms = ($end_time - ($env.MH_START_TIME | into int))
      if ("MH_RECORD_VERBOSE" in $env) {
        ^mh record --command $env.MH_LAST_COMMAND --cwd $env.PWD --shell nushell --exit-code $exit_code --duration-ms $duration_ms --session-id $env.MH_SESSION_ID out> stderr err> stderr
      } else {
        ^mh record --command $env.MH_LAST_COMMAND --cwd $env.PWD --shell nushell --exit-code $exit_code --duration-ms $duration_ms --session-id $env.MH_SESSION_ID | ignore
      }
      hide-env MH_LAST_COMMAND
      hide-env MH_START_TIME
    }
  }])
})

let mh_pick_limit = ($env.MH_PICK_LIMIT? | default 100)
let mh_history_picker_keybinding = {
  name: mh_history_picker
  modifier: none
  keycode: up
  mode: [emacs vi_normal vi_insert]
  event: {
    send: executehostcommand
    cmd: $"let selected = (^mh pick --limit ($mh_pick_limit | into string)); if (($selected | str length) > 0) { commandline edit --replace $selected }"
  }
}

let mh_existing_keybindings = ($env.config | get -i keybindings | default [])
$env.config = ($env.config | upsert keybindings (
  ($mh_existing_keybindings | where name != "mh_history_picker") ++ [$mh_history_picker_keybinding]
))
"#;
