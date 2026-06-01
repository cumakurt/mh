//! PowerShell 7+ integration via PSReadLine `AddToHistoryHandler`.

pub const INTEGRATION: &str = r#"# mh shell integration for PowerShell 7+ (pwsh)
# Add to $PROFILE (e.g. ~/.config/powershell/Microsoft.PowerShell_profile.ps1)

if (-not $global:__mh_pwsh_loaded) {
  $global:__mh_pwsh_loaded = $true

  if (-not $env:MH_SESSION_ID) {
    $env:MH_SESSION_ID = "{0}-{1}" -f [int][double]::Parse((Get-Date -UFormat %s)), $PID
  }
  $env:MH_SKIP_GIT_DETECT = "1"

  function global:__mh_record_command {
    param([string]$Line, [int]$ExitCode = 0, [int]$DurationMs = 0)
    if ([string]::IsNullOrWhiteSpace($Line)) { return }
    if ($Line -match '^(mh\s|__mh_)') { return }
    $mh = Get-Command mh -ErrorAction SilentlyContinue
    if (-not $mh) { return }
    $args = @(
      'record',
      '--command', $Line,
      '--cwd', (Get-Location).Path,
      '--shell', 'pwsh',
      '--exit-code', $ExitCode,
      '--duration-ms', $DurationMs,
      '--session-id', $env:MH_SESSION_ID
    )
    if ($env:MH_RECORD_VERBOSE -or $env:MH_POLICY_VERBOSE) {
      & mh @args 2>&1 | Write-Host
    } else {
      & mh @args 1>$null 2>$null
    }
  }

  if (Get-Module -ListAvailable PSReadLine) {
    Import-Module PSReadLine -ErrorAction SilentlyContinue
    Set-PSReadLineOption -AddToHistoryHandler {
      param([string]$line)
      __mh_record_command -Line $line -ExitCode $LASTEXITCODE
      return $line
    }
  } else {
    Write-Warning 'mh: PSReadLine module not found; install PowerShell 7+ or record manually with mh record'
  }
}
"#;
